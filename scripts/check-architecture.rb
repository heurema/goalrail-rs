#!/usr/bin/env ruby

require "json"
require "open3"
require "pathname"
require_relative "check-skills-boundary"

EXPECTED_WORKSPACE_MEMBERS = %w[gr gr-inspect-codex gr-site].freeze
EXPECTED_OWNED_EDGES = ["gr --normal--> gr-inspect-codex"].freeze
EXPECTED_PUBLIC_SURFACE = [
  "lib.rs: pub const fn as_str(self) -> &'static str",
  "lib.rs: pub const fn exit_code(self) -> u8",
  "lib.rs: pub enum Verdict",
  "lib.rs: pub use skills::{SkillsInspectionOutcome, inspect_codex_skill_actions, inspect_codex_skills};",
  "lib.rs: pub use use_case::{InspectionOutcome, inspect_codex};",
  "skills.rs: pub const fn is_failure(&self) -> bool",
  "skills.rs: pub const fn verdict(&self) -> Verdict",
  "skills.rs: pub fn inspect_codex_skill_actions() -> SkillsInspectionOutcome",
  "skills.rs: pub fn inspect_codex_skills() -> SkillsInspectionOutcome",
  "skills.rs: pub fn to_human(&self) -> String",
  "skills.rs: pub fn to_pretty_json(&self) -> Result<String, serde_json::Error>",
  "skills.rs: pub struct SkillsInspectionOutcome",
  "use_case.rs: pub const fn is_failure(&self) -> bool",
  "use_case.rs: pub const fn verdict(&self) -> Verdict",
  "use_case.rs: pub fn inspect_codex() -> InspectionOutcome",
  "use_case.rs: pub fn to_human(&self) -> String",
  "use_case.rs: pub fn to_pretty_json(&self) -> Result<String, serde_json::Error>",
  "use_case.rs: pub struct InspectionOutcome",
].sort.freeze
EXPECTED_VERDICT_VARIANTS = %w[BaselineOk Blocked Incomplete Review].freeze

class ArchitectureInputError < StandardError; end

def load_metadata(root)
  metadata_file = ENV["GOALRAIL_ARCHITECTURE_METADATA_FILE"]
  return File.read(metadata_file) if metadata_file && !metadata_file.empty?

  stdout, stderr, status = Open3.capture3(
    "cargo",
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    chdir: root.to_s,
  )
  raise ArchitectureInputError, "cargo metadata failed: #{stderr.strip}" unless status.success?

  stdout
rescue Errno::ENOENT => error
  raise ArchitectureInputError, "architecture input is unavailable: #{error.message}"
end

def parse_metadata(payload)
  JSON.parse(payload)
rescue JSON::ParserError => error
  raise ArchitectureInputError, "metadata JSON is invalid: #{error.message}"
end

def owned_architecture(metadata)
  packages = metadata.fetch("packages")
  members = packages.map { |package| package.fetch("name") }.sort
  edges = packages.each_with_object([]) do |package, owned_edges|
    package.fetch("dependencies").each do |dependency|
      next unless dependency["path"] && members.include?(dependency.fetch("name"))

      kind = dependency["kind"] || "normal"
      owned_edges << "#{package.fetch("name")} --#{kind}--> #{dependency.fetch("name")}"
    end
  end.sort

  [members, edges]
rescue KeyError, NoMethodError => error
  raise ArchitectureInputError,
    "metadata JSON does not match Cargo format 1: #{error.message}"
end

def collapse_whitespace(value)
  value.split.join(" ")
end

def declaration_mode(first_line)
  return :semicolon if first_line.match?(
    /\Apub (?:use|type|static|mod|extern crate)\b|\Apub const(?!\s+fn\b)/,
  )
  return :comma if first_line.match?(/\Apub [A-Za-z_][A-Za-z0-9_]*\s*:/)

  :body
end

def collect_public_declaration(lines, start)
  mode = declaration_mode(lines.fetch(start).strip)
  declaration = +""
  parentheses = 0
  brackets = 0
  braces = 0
  angles = 0

  lines[start..].each_with_index do |raw_line, offset|
    raw_line.each_char do |character|
      at_top_level = parentheses.zero? && brackets.zero? && braces.zero? &&
        angles.zero?
      terminates = (mode == :body && character == "{" && at_top_level) ||
        (mode == :semicolon && character == ";" && at_top_level) ||
        (mode == :comma && character == "," && at_top_level)
      if terminates
        declaration << character unless mode == :body
        return [collapse_whitespace(declaration), start + offset]
      end

      declaration << character
      case character
      when "("
        parentheses += 1
      when ")"
        parentheses -= 1 if parentheses.positive?
      when "["
        brackets += 1
      when "]"
        brackets -= 1 if brackets.positive?
      when "{"
        braces += 1
      when "}"
        braces -= 1 if braces.positive?
      when "<"
        angles += 1
      when ">"
        angles -= 1 if angles.positive?
      end
    end
    declaration << " "
  end

  [collapse_whitespace(declaration), lines.length - 1]
end

def public_declarations(source_root)
  declarations = []
  source_files = Dir.glob(source_root.join("**", "*.rs").to_s).sort
  raise ArchitectureInputError, "facade source is unavailable: #{source_root}" if source_files.empty?

  source_files.each do |path|
    relative_path = Pathname.new(path).relative_path_from(source_root).to_s
    lines = File.readlines(path)
    index = 0
    while index < lines.length
      line = lines.fetch(index).strip
      if line.start_with?("pub(crate)", "pub(super)", "pub(in ") ||
          !line.start_with?("pub ")
        index += 1
        next
      end

      declaration, terminal_index = collect_public_declaration(lines, index)
      declarations << "#{relative_path}: #{declaration}"
      index = terminal_index + 1
    end
  end

  declarations.sort
rescue Errno::ENOENT => error
  raise ArchitectureInputError, "facade source is unavailable: #{error.message}"
end

def enum_variants(path, enum_name)
  lines = File.readlines(path)
  start = lines.index { |line| line.match?(/^pub enum #{Regexp.escape(enum_name)}\s*\{/) }
  unless start
    raise ArchitectureInputError, "public enum #{enum_name} is missing from #{path}"
  end

  variants = []
  lines[(start + 1)..].each do |raw_line|
    line = raw_line.strip
    break if line == "}"
    next if line.empty? || line.start_with?("#", "//")

    match = line.match(/\A([A-Z][A-Za-z0-9_]*)\s*(?:,|\(|\{)/)
    variants << match[1] if match
  end
  variants.sort
rescue Errno::ENOENT => error
  raise ArchitectureInputError, "facade source is unavailable: #{error.message}"
end

def describe_difference(label, expected, actual)
  missing = expected - actual
  unexpected = actual - expected
  details = []
  details << "missing: #{missing.join(", ")}" unless missing.empty?
  details << "unexpected: #{unexpected.join(", ")}" unless unexpected.empty?
  "#{label} changed (#{details.join("; ")})"
end

def status(passed, pass_message, fail_message)
  passed ? "PASS - #{pass_message}" : "FAILED - #{fail_message}"
end

def print_receipt(ad3:, ad4:, ad5:, ad6:, aggregate:, details: [])
  output = aggregate == "FAILED" ? $stderr : $stdout
  output.puts "AD-1: MANUAL - CLI semantic ownership is not automated in v0"
  output.puts "AD-2: MANUAL - library orchestration ownership is not automated in v0"
  output.puts "AD-3: #{ad3}"
  output.puts "AD-4: #{ad4}"
  output.puts "AD-5: #{ad5}"
  output.puts "AD-6: #{ad6}"
  suffix = case aggregate
  when "PASS"
    " (automated scope)"
  when "REVIEW"
    " (AD-6 semantic enforcement pending)"
  else
    ""
  end
  output.puts "Architecture fitness v0: #{aggregate}#{suffix}"
  details.each { |detail| output.puts "- #{detail}" }
end

root = Pathname.new(
  ENV.fetch("GOALRAIL_ARCHITECTURE_ROOT", File.expand_path("..", __dir__)),
).expand_path
ad6 = SkillsBoundary.check(root)

begin
  metadata = parse_metadata(load_metadata(root))
  members, edges = owned_architecture(metadata)
rescue ArchitectureInputError => error
  print_receipt(
    ad3: "NOT_RUN - architecture input failed",
    ad4: "NOT_RUN - architecture input failed",
    ad5: "NOT_RUN - architecture input failed",
    ad6: ad6.receipt,
    aggregate: "FAILED",
    details: [error.message] + ad6.details,
  )
  exit 1
end

ad3_violations = []
unless members == EXPECTED_WORKSPACE_MEMBERS
  ad3_violations << describe_difference(
    "AD-3 workspace members",
    EXPECTED_WORKSPACE_MEMBERS,
    members,
  )
end
unless edges == EXPECTED_OWNED_EDGES
  ad3_violations << describe_difference(
    "AD-3 owned dependency edges",
    EXPECTED_OWNED_EDGES,
    edges,
  )
end

site_isolation_passed = members.include?("gr-site") &&
  edges.none? { |edge| edge.start_with?("gr-site --") }

ad4_violations = []
source_root = root.join("crates", "gr-inspect-codex", "src")
begin
  actual_surface = public_declarations(source_root)
  unless actual_surface == EXPECTED_PUBLIC_SURFACE
    ad4_violations << describe_difference(
      "AD-4 public surface",
      EXPECTED_PUBLIC_SURFACE,
      actual_surface,
    )
  end

  actual_variants = enum_variants(source_root.join("lib.rs"), "Verdict")
  unless actual_variants == EXPECTED_VERDICT_VARIANTS
    ad4_violations << describe_difference(
      "AD-4 Verdict variants",
      EXPECTED_VERDICT_VARIANTS,
      actual_variants,
    )
  end
rescue ArchitectureInputError => error
  ad3_passed = ad3_violations.empty?
  print_receipt(
    ad3: status(
      ad3_passed,
      "workspace members and owned dependency edges match the spine",
      "workspace members or owned dependency edges changed",
    ),
    ad4: "NOT_RUN - facade input failed",
    ad5: status(
      site_isolation_passed,
      "gr-site has no owned dependency edge",
      "gr-site is missing or has an owned dependency edge",
    ),
    ad6: ad6.receipt,
    aggregate: "FAILED",
    details: ad3_violations + [error.message] + ad6.details,
  )
  exit 1
end

ad3_passed = ad3_violations.empty?
ad4_passed = ad4_violations.empty?
aggregate_passed = ad3_passed && ad4_passed && site_isolation_passed
details = ad3_violations + ad4_violations + ad6.details
unless site_isolation_passed
  details << "AD-5 gr-site isolation changed" unless details.any? { |item| item.include?("gr-site") }
end

print_receipt(
  ad3: status(
    ad3_passed,
    "workspace members and owned dependency edges match the spine",
    "workspace members or owned dependency edges changed",
  ),
  ad4: status(
    ad4_passed,
    "the full source-level public facade snapshot matches",
    "the source-level public facade or Verdict variants changed",
  ),
  ad5: status(
    site_isolation_passed,
    "gr-site has no owned dependency edge",
    "gr-site is missing or has an owned dependency edge",
  ),
  ad6: ad6.receipt,
  aggregate: if !aggregate_passed || ad6.failed?
    "FAILED"
  elsif ad6.status == "REVIEW"
    "REVIEW"
  else
    "PASS"
  end,
  details: details,
)

exit 1 unless aggregate_passed && !ad6.failed?

#!/usr/bin/env ruby

require "pathname"

module SkillsBoundary
  STAGES = %w[model catalog history assessment presentation].freeze
  ALLOWED_EDGES = {
    "model" => [],
    "catalog" => ["model"],
    "history" => ["model"],
    "assessment" => ["model"],
    "presentation" => ["assessment"],
  }.freeze
  PURITY_PATTERNS = [
    ["process access", /\b(?:std::process|process::Command|Command::new|Stdio)\b/],
    [
      "environment access",
      /\b(?:std::env|env::(?:args|args_os|current_dir|current_exe|home_dir|join_paths|set_current_dir|set_var|split_paths|temp_dir|var|var_os|vars|vars_os))\b/,
    ],
    [
      "filesystem access",
      /\b(?:std::fs|fs::|File::(?:create|open)|OpenOptions::new|canonicalize\s*\(|read_dir\s*\(|read_link\s*\(|read_to_string\s*\() /x,
    ],
    [
      "clock access",
      /\b(?:SystemTime|Instant|UNIX_EPOCH|Utc::now|Local::now|OffsetDateTime::now_utc)\b/,
    ],
    [
      "rendering",
      /\b(?:std::fmt::Write|fmt::Write|serde_json::to_(?:string|value|writer)|to_human|to_pretty_json)\b|\b(?:e?println|write|writeln)!/,
    ],
  ].freeze

  Result = Struct.new(:status, :message, :details, keyword_init: true) do
    def failed?
      status == "FAILED"
    end

    def receipt
      "#{status} - #{message}"
    end
  end

  module_function

  def module_declarations(source)
    declarations = []
    attributed_declarations = []
    brace_depth = 0
    attribute_depth = 0
    pending_attribute = false
    sanitized_source(source).each_line do |line|
      if brace_depth.zero?
        stripped = line.strip
        if attribute_depth.positive?
          attribute_depth += line.count("[") - line.count("]")
        elsif stripped.start_with?("#[")
          pending_attribute = true
          attribute_depth = line.count("[") - line.count("]")
        elsif !stripped.empty?
          match = line.match(
            /^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;/,
          )
          if match
            target = pending_attribute ? attributed_declarations : declarations
            target << match[1]
          end
          pending_attribute = false
        end
      end

      brace_depth += line.count("{") - line.count("}")
      brace_depth = 0 if brace_depth.negative?
    end
    [declarations.sort, attributed_declarations.sort]
  end

  def describe_set(label, expected, actual)
    details = []
    missing = expected - actual
    unexpected = actual - expected
    details << "missing #{label}: #{missing.join(", ")}" unless missing.empty?
    details << "unexpected #{label}: #{unexpected.join(", ")}" unless unexpected.empty?
    details
  end

  def sanitized_source(source)
    output = +""
    index = 0
    block_depth = 0
    state = :code
    raw_closer = nil

    while index < source.length
      if state == :line_comment
        if source[index] == "\n"
          output << "\n"
          state = :code
        else
          output << " "
        end
        index += 1
        next
      end

      if state == :block_comment
        if source[index, 2] == "/*"
          output << "  "
          block_depth += 1
          index += 2
        elsif source[index, 2] == "*/"
          output << "  "
          block_depth -= 1
          state = :code if block_depth.zero?
          index += 2
        else
          output << (source[index] == "\n" ? "\n" : " ")
          index += 1
        end
        next
      end

      if state == :string
        if source[index] == "\\"
          output << "  "
          index += 2
        elsif source[index] == '"'
          output << " "
          state = :code
          index += 1
        else
          output << (source[index] == "\n" ? "\n" : " ")
          index += 1
        end
        next
      end

      if state == :raw_string
        if source[index, raw_closer.length] == raw_closer
          output << (" " * raw_closer.length)
          index += raw_closer.length
          state = :code
        else
          output << (source[index] == "\n" ? "\n" : " ")
          index += 1
        end
        next
      end

      if source[index, 2] == "//"
        output << "  "
        state = :line_comment
        index += 2
      elsif source[index, 2] == "/*"
        output << "  "
        state = :block_comment
        block_depth = 1
        index += 2
      elsif source[index] == "'"
        char_match = source[index..].match(
          /\A'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,6}\}|.)|[^\\'\r\n])'/,
        )
        if char_match
          output << (" " * char_match[0].length)
          index += char_match[0].length
        else
          output << source[index]
          index += 1
        end
      elsif source[index] == '"'
        output << " "
        state = :string
        index += 1
      elsif source[index] == "r"
        raw_match = source[index..].match(/\Ar(#+)?"/)
        if raw_match
          hashes = raw_match[1] || ""
          opener_length = 2 + hashes.length
          output << (" " * opener_length)
          raw_closer = '"' + hashes
          state = :raw_string
          index += opener_length
        else
          output << source[index]
          index += 1
        end
      else
        output << source[index]
        index += 1
      end
    end

    output
  end

  def referenced_stages(source)
    code = sanitized_source(source)
    imports = code.scan(/^\s*use\s+(.+?);/m).flatten

    STAGES.select do |stage|
      imports.any? do |path|
        path.match?(/\b(?:super|crate::skills)::#{Regexp.escape(stage)}\b/) ||
          path.match?(
            /\b(?:super|crate::skills)::\{[^}]*\b#{Regexp.escape(stage)}\b/m,
          )
      end
    end
  end

  def dependency_violations(source_dir)
    STAGES.each_with_object([]) do |stage, violations|
      source = File.read(source_dir.join("#{stage}.rs"))
      referenced_stages(source).each do |dependency|
        next if dependency == stage || ALLOWED_EDGES.fetch(stage).include?(dependency)

        violations << "AD-6 forbidden edge: #{stage} -> #{dependency}"
      end
    end
  end

  def purity_violations(source_dir)
    source = sanitized_source(File.read(source_dir.join("assessment.rs")))
    PURITY_PATTERNS.each_with_object([]) do |(label, pattern), violations|
      match = source.match(pattern)
      next unless match

      token = match[0].split.join(" ")
      violations << "AD-6 assessment purity violation: #{label} via #{token}"
    end
  end

  def check(root)
    source_root = Pathname.new(root).join("crates", "gr-inspect-codex", "src")
    orchestrator = source_root.join("skills.rs")
    unless orchestrator.file?
      return Result.new(
        status: "FAILED",
        message: "skills orchestrator source is unavailable",
        details: [orchestrator.to_s],
      )
    end

    source_dir = source_root.join("skills")
    actual_files = if source_dir.directory?
      Dir.glob(source_dir.join("*.rs").to_s).map do |path|
        File.basename(path, ".rs")
      end.sort
    else
      []
    end
    actual_declarations, attributed_declarations = module_declarations(
      File.read(orchestrator),
    )

    if actual_files.empty? && actual_declarations.empty? &&
        attributed_declarations.empty?
      return Result.new(
        status: "REVIEW",
        message: "accepted boundary is not implemented",
        details: [],
      )
    end

    topology_violations = describe_set("module files", STAGES, actual_files) +
      describe_set("module declarations", STAGES, actual_declarations)
    unless attributed_declarations.empty?
      topology_violations <<
        "conditional or attributed module declarations are not accepted: " \
        "#{attributed_declarations.join(", ")}"
    end
    unless topology_violations.empty?
      return Result.new(
        status: "FAILED",
        message: "accepted boundary has a partial or unclassified module topology",
        details: topology_violations,
      )
    end

    violations = dependency_violations(source_dir) + purity_violations(source_dir)
    unless violations.empty?
      return Result.new(
        status: "FAILED",
        message: "provisional source-level boundary check rejected the skills modules",
        details: violations,
      )
    end

    Result.new(
      status: "REVIEW",
      message: "source-level contract conforms; semantic module graph pending",
      details: [],
    )
  rescue Errno::ENOENT => error
    Result.new(
      status: "FAILED",
      message: "skills boundary input is unavailable",
      details: [error.message],
    )
  end
end

if $PROGRAM_NAME == __FILE__
  root = Pathname.new(
    ENV.fetch("GOALRAIL_SKILLS_BOUNDARY_ROOT", File.expand_path("..", __dir__)),
  ).expand_path
  result = SkillsBoundary.check(root)
  output = result.failed? ? $stderr : $stdout
  output.puts "AD-6: #{result.receipt}"
  result.details.each { |detail| output.puts "- #{detail}" }
  exit 1 if result.failed?
end

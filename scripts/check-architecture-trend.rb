#!/usr/bin/env ruby

require "open3"
require "pathname"

REVISION_WINDOW = 3
TOP_LEVEL_ITEM = /
  \A
  (?:pub(?:\([^)]*\))?\s+)?
  (?:
    (?:(?:const|async|unsafe)\s+)*(?:extern(?:\s+"[^"]+")?\s+)?fn\s+
    |struct\s+
    |enum\s+
    |union\s+
    |(?:unsafe\s+)?(?:auto\s+)?trait\s+
    |(?:unsafe\s+)?impl(?:<|\s)
    |type\s+
    |const\s+
    |static\s+
    |mod\s+
  )
/x

class TrendInputError < StandardError; end

def source_line_count(content)
  content.lines.count
end

def top_level_item_count(content)
  content.each_line.count { |line| line.match?(TOP_LEVEL_ITEM) }
end

def median(values)
  sorted = values.sort
  midpoint = sorted.length / 2
  return sorted.fetch(midpoint).to_f if sorted.length.odd?

  (sorted.fetch(midpoint - 1) + sorted.fetch(midpoint)).fdiv(2)
end

def tukey_upper_fence(values)
  return nil if values.length < 4

  sorted = values.sort
  midpoint = sorted.length / 2
  lower = sorted[0...midpoint]
  upper = sorted[(sorted.length.odd? ? midpoint + 1 : midpoint)..]
  first_quartile = median(lower)
  third_quartile = median(upper)
  interquartile_range = third_quartile - first_quartile
  return nil unless interquartile_range.positive?

  third_quartile + (1.5 * interquartile_range)
end

def git(root, *arguments)
  stdout, stderr, status = Open3.capture3("git", *arguments, chdir: root.to_s)
  unless status.success?
    raise TrendInputError, "git #{arguments.first} failed: #{stderr.strip}"
  end

  stdout
rescue Errno::ENOENT => error
  raise TrendInputError, "git history is unavailable: #{error.message}"
end

def revision_contents(root, relative_path, current_content)
  hashes = git(
    root,
    "log",
    "--follow",
    "--format=%H",
    "--",
    relative_path,
  ).lines.map(&:strip).reject(&:empty?)

  observations = []
  newest_content = hashes.empty? ? nil : git(root, "show", "#{hashes.first}:#{relative_path}")
  worktree_changed = newest_content != current_content
  history_limit = worktree_changed ? REVISION_WINDOW - 1 : REVISION_WINDOW

  hashes.take(history_limit).reverse_each do |commit|
    observations << {
      label: commit[0, 7],
      content: git(root, "show", "#{commit}:#{relative_path}"),
    }
  end
  if worktree_changed
    observations << { label: "WORKTREE", content: current_content }
  end

  observations.last(REVISION_WINDOW)
end

def strictly_growing?(values)
  values.length == REVISION_WINDOW &&
    values.each_cons(2).all? { |before, after| after > before }
end

def source_groups(root)
  groups = Hash.new { |hash, key| hash[key] = [] }
  Dir.glob(root.join("crates", "*", "src", "**", "*.rs").to_s).sort.each do |path|
    source_path = Pathname.new(path)
    relative = source_path.relative_path_from(root)
    source_root = root.join(*relative.each_filename.take(3))
    groups[source_root] << source_path
  end
  groups
end

def format_number(value)
  value == value.to_i ? value.to_i.to_s : format("%.1f", value)
end

root = Pathname.new(
  ENV.fetch("GOALRAIL_ARCHITECTURE_TREND_ROOT", File.expand_path("..", __dir__)),
).expand_path

begin
  signals = []
  source_groups(root).sort_by { |source_root, _| source_root.to_s }.each do |source_root, paths|
    sizes = paths.map { |path| File.readlines(path).length }
    upper_fence = tukey_upper_fence(sizes)
    next unless upper_fence

    paths.each do |path|
      current_content = File.read(path)
      current_lines = source_line_count(current_content)
      next unless current_lines > upper_fence

      relative_path = path.relative_path_from(root).to_s
      revisions = revision_contents(root, relative_path, current_content)
      line_history = revisions.map { |revision| source_line_count(revision.fetch(:content)) }
      item_history = revisions.map { |revision| top_level_item_count(revision.fetch(:content)) }
      repeated_growth = strictly_growing?(line_history) && strictly_growing?(item_history)
      signals << {
        status: repeated_growth ? "REVIEW" : "OBSERVE",
        path: relative_path,
        fence: upper_fence,
        labels: revisions.map { |revision| revision.fetch(:label) },
        lines: line_history,
        items: item_history,
      }
    end
  end
rescue TrendInputError, SystemCallError => error
  warn "Architecture trend v0: INCOMPLETE - #{error.message}"
  exit 0
end

signals.sort_by { |signal| [signal.fetch(:status), signal.fetch(:path)] }.each do |signal|
  puts "- #{signal.fetch(:status)} #{signal.fetch(:path)}"
  puts "  revisions: #{signal.fetch(:labels).join(" -> ")}"
  puts "  source_lines: #{signal.fetch(:lines).join(" -> ")}"
  puts "  top_level_items: #{signal.fetch(:items).join(" -> ")}"
  puts "  crate_tukey_upper_fence: #{format_number(signal.fetch(:fence))} source lines"
end

aggregate = if signals.any? { |signal| signal.fetch(:status) == "REVIEW" }
  "REVIEW"
elsif signals.empty?
  "NO_REVIEW_SIGNAL"
else
  "OBSERVE"
end
puts "Architecture trend v0: #{aggregate} (advisory; hard gate unaffected)"

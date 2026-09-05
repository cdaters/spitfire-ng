#!/usr/bin/env ruby
# frozen_string_literal: true
# SPITFIRE NG
# Preservation-driven modern cross-platform reimplementation of
# Buffalo Creek Software's SPITFIRE Bulletin Board System
#
# Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
# Licensed under MIT OR Apache-2.0
#
# This file is part of the SPITFIRE NG project.
# See the repository documentation for architecture, provenance,
# compatibility research, security, and contribution guidelines.

require "English"

# This validator deliberately covers only reviewed project-source locations.
# Cargo manifests, Cargo.lock, workflow configuration, resources, fixtures,
# generated output, vendored code, and historical samples are outside these
# scopes and must not acquire a project-ownership header by inference.

HEADER_BODY = [
  "SPITFIRE NG",
  "Preservation-driven modern cross-platform reimplementation of",
  "Buffalo Creek Software's SPITFIRE Bulletin Board System",
  "",
  "Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors",
  "Licensed under MIT OR Apache-2.0",
  "",
  "This file is part of the SPITFIRE NG project.",
  "See the repository documentation for architecture, provenance,",
  "compatibility research, security, and contribution guidelines."
].freeze

ROOT = File.expand_path("..", __dir__)
SCOPES = {
  rust: "crates/*/{src,tests}/**/*.rs",
  shell: "tools/*.sh",
  powershell: "tools/*.ps1",
  ruby: "tools/*.rb"
}.freeze

# These tracked files are code-adjacent, but the reviewed policy excludes them
# from per-file headers. Cargo manifests already inherit the workspace license;
# Cargo.lock is generated dependency state; and the issue form is repository
# configuration rather than executable source.
EXCLUDED_NON_SOURCE_PATHS = {
  ".github/ISSUE_TEMPLATE/bug-report.yml" => "GitHub issue-form configuration",
  "Cargo.lock" => "generated dependency lockfile",
  "Cargo.toml" => "workspace manifest with explicit license metadata",
  "crates/sf-bbs/Cargo.toml" => "crate manifest inheriting workspace license",
  "crates/sf-core/Cargo.toml" => "crate manifest inheriting workspace license",
  "crates/sf-net/Cargo.toml" => "crate manifest inheriting workspace license",
  "crates/sf-legacy/Cargo.toml" => "crate manifest inheriting workspace license"
}.freeze

# No tracked source-shaped file is currently excluded: all such files in the
# reviewed scopes are independently authored SPITFIRE NG source. A future
# generated, vendored, third-party, or historical source file must be added
# here with a specific reason before the validator will accept it.
EXCLUDED_SOURCE_PATHS = {}.freeze

EXCLUDED_CONTENT_PREFIXES = {
  "crates/sf-core/i18n/" => "localized resource data",
  "research/samples/" => "historical evidence and its tracked boundary guide",
  "release/" => "release metadata and generated-package source material"
}.freeze

def scoped_candidates
  SCOPES.flat_map do |kind, pattern|
    Dir.glob(File.join(ROOT, pattern)).sort.map { |path| [kind, path] }
  end.uniq
end

def candidates
  scoped_candidates.reject do |_kind, path|
    EXCLUDED_SOURCE_PATHS.key?(path.delete_prefix("#{ROOT}/"))
  end
end

def tracked_paths
  @tracked_paths ||= begin
    output = IO.popen(["git", "-C", ROOT, "ls-files"], &:read)
    abort "could not inventory tracked files with git" unless $CHILD_STATUS.success?

    output.lines(chomp: true).sort
  end
end

def tracked_source_paths
  tracked_paths.select do |path|
    %w[.rs .sh .ps1 .rb].include?(File.extname(path).downcase)
  end
end

def comment_prefix(kind)
  kind == :rust ? "//" : "#"
end

def header_lines(kind)
  prefix = comment_prefix(kind)
  HEADER_BODY.map { |line| line.empty? ? prefix : "#{prefix} #{line}" }
end

def insertion_index(kind, lines, relative_path)
  case kind
  when :rust, :powershell
    0
  when :shell
    abort "#{relative_path}: shell script must retain a shebang on line 1" unless lines[0]&.start_with?("#!")

    1
  when :ruby
    abort "#{relative_path}: Ruby tool must retain a shebang on line 1" unless lines[0]&.start_with?("#!")

    index = 1
    while lines[index]&.match?(/\A# (?:frozen_string_literal:|encoding:|coding:|SPDX-License-Identifier:)/)
      index += 1
    end
    index
  else
    abort "#{relative_path}: unsupported validator classification #{kind}"
  end
end

def compliant?(kind, lines, relative_path)
  index = insertion_index(kind, lines, relative_path)
  lines[index, HEADER_BODY.length] == header_lines(kind)
end

fix = ARGV.delete("--fix")
abort "usage: tools/verify-source-headers.rb [--fix]" unless ARGV.empty?

counts = Hash.new(0)
failures = []
candidate_paths = candidates.map { |_kind, path| path.delete_prefix("#{ROOT}/") }.sort
scoped_paths = scoped_candidates.map { |_kind, path| path.delete_prefix("#{ROOT}/") }.sort
unclassified = tracked_source_paths - candidate_paths - EXCLUDED_SOURCE_PATHS.keys
unclassified.each do |path|
  failures << "#{path}: tracked source is outside the reviewed header scopes"
end

missing_exclusions = EXCLUDED_NON_SOURCE_PATHS.keys.reject do |path|
  File.file?(File.join(ROOT, path))
end
missing_exclusions.each do |path|
  failures << "#{path}: reviewed non-source exclusion no longer exists"
end

invalid_source_exclusions = EXCLUDED_SOURCE_PATHS.keys - scoped_paths
invalid_source_exclusions.each do |path|
  failures << "#{path}: source exclusion is outside the reviewed scopes or no longer exists"
end

content_scope_overlap = scoped_paths.select do |path|
  EXCLUDED_CONTENT_PREFIXES.keys.any? { |prefix| path.start_with?(prefix) }
end
content_scope_overlap.each do |path|
  failures << "#{path}: content-excluded path cannot be in a required source scope"
end

excluded_content_paths = EXCLUDED_NON_SOURCE_PATHS.keys + tracked_paths.select do |path|
  EXCLUDED_CONTENT_PREFIXES.keys.any? { |prefix| path.start_with?(prefix) }
end
excluded_content_paths.uniq.each do |path|
  content = File.binread(File.join(ROOT, path))
  if content.include?(HEADER_BODY.fetch(4))
    failures << "#{path}: excluded content carries the project source header"
  end
end

candidates.each do |kind, path|
  relative_path = path.delete_prefix("#{ROOT}/")
  content = File.binread(path)
  abort "#{relative_path}: source must be UTF-8 text" unless content.dup.force_encoding(Encoding::UTF_8).valid_encoding?

  lines = content.lines(chomp: true)
  if compliant?(kind, lines, relative_path)
    counts[kind] += 1
    next
  end

  unless fix
    failures << "#{relative_path}: canonical header missing or misplaced"
    next
  end

  if content.include?("Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors")
    abort "#{relative_path}: found a malformed or misplaced project header; review manually"
  end

  index = insertion_index(kind, lines, relative_path)
  lines.insert(index, *header_lines(kind))
  after_header = index + HEADER_BODY.length
  lines.insert(after_header, "") unless lines[after_header]&.empty?
  File.binwrite(path, "#{lines.join("\n")}\n")
  counts[kind] += 1
end

unless failures.empty?
  warn failures.join("\n")
  exit 1
end

total = counts.values.sum
summary = SCOPES.keys.map { |kind| "#{counts[kind]} #{kind}" }.join(", ")
puts "Verified #{total} project-authored source headers (#{summary})."
puts "Reviewed #{EXCLUDED_NON_SOURCE_PATHS.length} explicit non-source paths, " \
     "#{EXCLUDED_SOURCE_PATHS.length} source exclusions, and " \
     "#{EXCLUDED_CONTENT_PREFIXES.length} content exclusion classes."

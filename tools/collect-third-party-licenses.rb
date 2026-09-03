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

require "fileutils"
require "json"

unless ARGV.length == 4
  abort "usage: collect-third-party-licenses.rb METADATA.json OUTPUT-DIRECTORY LICENSE-MIT LICENSE-APACHE"
end

metadata = JSON.parse(File.read(ARGV[0]))
output = File.expand_path(ARGV[1])
mit_template = File.read(ARGV[2])
apache_template = File.read(ARGV[3])
packages = metadata.fetch("packages").to_h { |package| [package.fetch("id"), package] }
nodes = metadata.fetch("resolve").fetch("nodes").to_h { |node| [node.fetch("id"), node] }
root = metadata.fetch("packages").find do |package|
  package.fetch("name") == "sf-bbs" && package["source"].nil?
end
abort "sf-bbs workspace package is missing from cargo metadata" unless root

seen = {}
pending = [root.fetch("id")]
until pending.empty?
  id = pending.pop
  next if seen[id]

  seen[id] = true
  node = nodes.fetch(id)
  node.fetch("deps").each { |dependency| pending << dependency.fetch("pkg") }
end

third_party = seen.keys.map do |id|
  package = packages.fetch(id)
  package if package["source"]
end.compact.sort_by { |package| [package.fetch("name"), package.fetch("version")] }

FileUtils.mkdir_p(output)
index = [
  "# Third-Party Notices",
  "",
  "This inventory is generated from the locked dependency graph for the release target.",
  "Each listed directory contains the upstream license/notice files shipped with that crate.",
  "Those files retain their original terms; the SPITFIRE NG project license does not replace them.",
  "",
  "| Crate | Version | Declared license | Upstream | Included notices |",
  "|---|---:|---|---|---|"
]

third_party.each do |package|
  name = package.fetch("name")
  version = package.fetch("version")
  license = package["license"]
  abort "#{name} #{version} has no declared license" if license.nil? || license.empty?

  source_directory = File.dirname(package.fetch("manifest_path"))
  candidates = Dir.children(source_directory).select do |entry|
    entry.match?(/\A(?:LICENSE|COPYING|NOTICE|UNLICENSE)/i) &&
      File.file?(File.join(source_directory, entry))
  end.sort
  package_directory = File.join(output, "#{name}-#{version}")
  FileUtils.mkdir_p(package_directory)
  if candidates.empty?
    authors = package.fetch("authors", []).join(", ")
    authors = "upstream #{name} contributors" if authors.empty?
    metadata_notice = [
      "Upstream package: #{name} #{version}",
      "Declared license: #{license}",
      "Authors: #{authors}",
      "Source: #{package.fetch("source")}",
      "Repository: #{package["repository"] || package["homepage"] || "not declared"}",
      "",
      "The published crate did not contain a standalone license file.",
      "The applicable standard text below is included from its declared Cargo license metadata."
    ]
    File.write(
      File.join(package_directory, "UPSTREAM-PACKAGE-METADATA.txt"),
      metadata_notice.join("\n")
    )
    candidates << "UPSTREAM-PACKAGE-METADATA.txt"
    if license.match?(/\bMIT\b/)
      upstream_mit = mit_template.sub(/^Copyright.*$/, "Copyright (c) #{authors}")
      File.write(File.join(package_directory, "LICENSE-MIT"), upstream_mit)
      candidates << "LICENSE-MIT"
    end
    if license.match?(/Apache-2\.0/)
      File.write(File.join(package_directory, "LICENSE-APACHE"), apache_template)
      candidates << "LICENSE-APACHE"
    end
    if candidates.length == 1
      abort "#{name} #{version} has no packaged notice and no supported standard fallback for #{license}"
    end
  end
  candidates.each do |entry|
    source = File.join(source_directory, entry)
    FileUtils.cp(source, File.join(package_directory, entry)) if File.file?(source)
  end
  upstream = package["repository"] || package["homepage"] || package.fetch("source")
  notices = candidates.map { |entry| "`#{name}-#{version}/#{entry}`" }.join("<br>")
  index << "| #{name} | #{version} | #{license} | #{upstream} | #{notices} |"
end

index << ""
File.write(File.join(output, "THIRD-PARTY-NOTICES.md"), index.join("\n"))
puts "Collected #{third_party.length} locked third-party packages"

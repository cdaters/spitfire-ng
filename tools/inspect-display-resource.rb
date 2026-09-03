#!/usr/bin/env ruby
# SPDX-License-Identifier: MIT OR Apache-2.0
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

require "digest"
require "optparse"
require "tmpdir"

module DisplayResourceInspector
  module_function

  CSI = /\x1b\[[0-?]*[ -\/]*[@-~]/n

  def analyze(path)
    bytes = File.binread(path)
    crlf = bytes.scan(/\r\n/n).length
    controls = Hash.new { |hash, key| hash[key] = [] }
    bytes.bytes.each_with_index do |byte, offset|
      controls[byte] << offset if byte < 0x20 || byte == 0x7f
    end
    sequences = []
    bytes.to_enum(:scan, CSI).each do
      match = Regexp.last_match
      sequences << [match.begin(0), match[0].bytes]
    end
    physical_lines = bytes.split(/\r\n|\r|\n/n, -1)
    sauce_offset = bytes.bytesize >= 128 && bytes.byteslice(-128, 7) == "SAUCE00" ? bytes.bytesize - 128 : nil

    {
      path: path,
      size: bytes.bytesize,
      sha256: Digest::SHA256.hexdigest(bytes),
      extension: File.extname(path).delete_prefix(".").upcase,
      crlf: crlf,
      bare_cr: bytes.count("\r") - crlf,
      bare_lf: bytes.count("\n") - crlf,
      longest_physical_line: physical_lines.map(&:bytesize).max || 0,
      esc_offsets: controls[0x1b],
      csi_sequences: sequences,
      high_bit_count: bytes.each_byte.count { |byte| byte >= 0x80 },
      high_bit_values: bytes.each_byte.select { |byte| byte >= 0x80 }.uniq.sort,
      utf8_valid: bytes.dup.force_encoding(Encoding::UTF_8).valid_encoding?,
      bom: byte_order_mark(bytes),
      nul_offsets: controls[0x00],
      dos_eof_offsets: bytes.bytes.each_index.select { |offset| bytes.getbyte(offset) == 0x1a },
      control_offsets: controls,
      clear_markers: clear_markers(bytes),
      home_markers: home_markers(sequences),
      sauce_offset: sauce_offset,
      trailing_crlf: bytes.end_with?("\r\n"),
      trailing_hex: bytes.byteslice([bytes.bytesize - 16, 0].max, 16).unpack1("H*")
    }
  end

  def byte_order_mark(bytes)
    return "UTF-32BE" if bytes.start_with?("\x00\x00\xFE\xFF".b)
    return "UTF-32LE" if bytes.start_with?("\xFF\xFE\x00\x00".b)
    return "UTF-8" if bytes.start_with?("\xEF\xBB\xBF".b)
    return "UTF-16BE" if bytes.start_with?("\xFE\xFF".b)
    return "UTF-16LE" if bytes.start_with?("\xFF\xFE".b)

    nil
  end

  def clear_markers(bytes)
    markers = []
    markers << "SPITFIRE ^L" if bytes.include?("\x0c")
    markers << "SPITFIRE @CLS@" if bytes.include?("@CLS@")
    markers << "ANSI CSI 2 J" if bytes.include?("\x1b[2J")
    markers << "RIP !|1K" if bytes.include?("!|1K")
    markers
  end

  def home_markers(sequences)
    sequences.map do |offset, sequence|
      text = sequence.pack("C*")
      next unless ["\x1b[H", "\x1b[1H", "\x1b[1;1H", "\x1b[;H"].include?(text)

      format("ANSI home at 0x%X", offset)
    end.compact
  end

  def render(report)
    puts report[:path]
    puts "  type: .#{report[:extension]}"
    puts "  size: #{report[:size]} bytes"
    puts "  sha256: #{report[:sha256]}"
    puts "  line endings: CRLF=#{report[:crlf]} bare-CR=#{report[:bare_cr]} bare-LF=#{report[:bare_lf]}"
    puts "  longest physical line: #{report[:longest_physical_line]} bytes"
    puts "  ANSI ESC bytes: #{report[:esc_offsets].length}"
    puts "  ANSI CSI sequences: #{report[:csi_sequences].length}"
    report[:csi_sequences].each do |offset, sequence|
      puts format("    0x%X: %s", offset, sequence.map { |byte| format("%02X", byte) }.join(" "))
    end
    puts "  high-bit bytes: #{report[:high_bit_count]} (values: #{hex_values(report[:high_bit_values])})"
    puts "  valid UTF-8 stream: #{report[:utf8_valid] ? 'yes' : 'no'}"
    puts "  byte-order mark: #{report[:bom] || 'none'}"
    noteworthy_controls = report[:control_offsets].reject do |byte, offsets|
      offsets.empty? || [0x00, 0x0a, 0x0d, 0x1b].include?(byte)
    end
    if noteworthy_controls.empty?
      puts "  other control bytes: none"
    else
      puts "  other control bytes:"
      noteworthy_controls.sort.each do |byte, offsets|
        puts format("    %02X: %s", byte, hex_offsets(offsets))
      end
    end
    puts "  NUL offsets: #{hex_offsets(report[:nul_offsets])}"
    puts "  DOS EOF (0x1A) offsets: #{hex_offsets(report[:dos_eof_offsets])}"
    puts "  clear markers: #{list(report[:clear_markers])}"
    puts "  home markers: #{list(report[:home_markers])}"
    puts "  SAUCE record: #{report[:sauce_offset] ? format('yes, at 0x%X', report[:sauce_offset]) : 'none'}"
    puts "  trailing CRLF: #{report[:trailing_crlf] ? 'yes' : 'no'}"
    puts "  trailing bytes (hex): #{report[:trailing_hex]}"
  end

  def hex_offsets(offsets)
    offsets.empty? ? "none" : offsets.map { |offset| format("0x%X", offset) }.join(", ")
  end

  def hex_values(values)
    values.empty? ? "none" : values.map { |value| format("%02X", value) }.join(" ")
  end

  def list(values)
    values.empty? ? "none" : values.join(", ")
  end

  def self_test
    Dir.mktmpdir("display-inspector") do |directory|
      path = File.join(directory, "TEST.CLR")
      File.binwrite(path, "@CLS@\x1b[31m\xDB\r\n\x1a".b)
      report = analyze(path)
      raise "size" unless report[:size] == 14
      raise "CRLF" unless report[:crlf] == 1 && report[:bare_lf].zero?
      raise "CSI" unless report[:csi_sequences].length == 1
      raise "CP437" unless report[:high_bit_values] == [0xdb]
      raise "UTF-8" if report[:utf8_valid]
      raise "BOM" unless report[:bom].nil?
      raise "EOF" unless report[:dos_eof_offsets] == [13]
      raise "clear" unless report[:clear_markers].include?("SPITFIRE @CLS@")
      raise "SAUCE" unless report[:sauce_offset].nil?
    end
    puts "display resource inspector self-test: PASS"
  end
end

self_test = false
OptionParser.new do |options|
  options.banner = "Usage: ruby tools/inspect-display-resource.rb [--self-test] FILE..."
  options.on("--self-test", "Run the deterministic built-in test") { self_test = true }
end.parse!

if self_test
  DisplayResourceInspector.self_test
  exit 0 if ARGV.empty?
end

abort "no display files supplied" if ARGV.empty?
ARGV.each_with_index do |path, index|
  puts if index.positive?
  DisplayResourceInspector.render(DisplayResourceInspector.analyze(path))
end

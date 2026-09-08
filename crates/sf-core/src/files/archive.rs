// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.

//! Bounded inspection without extracting archive-supplied paths.
use super::scanner::{ScanReport, ScanResult, Scanner};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaType {
    Zip,
    Tar,
    Gzip,
    SevenZip,
    Rar,
    Bzip2,
    Xz,
    Zstandard,
    Arc,
    Arj,
    Lha,
    Executable,
    Png,
    Jpeg,
    Pdf,
    Text,
    Binary,
}
impl MediaType {
    fn unsupported(self) -> bool {
        matches!(
            self,
            Self::SevenZip
                | Self::Rar
                | Self::Bzip2
                | Self::Xz
                | Self::Zstandard
                | Self::Arc
                | Self::Arj
                | Self::Lha
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveLimits {
    pub source_bytes: u64,
    pub expanded_bytes: u64,
    pub member_bytes: u64,
    pub members: u32,
    pub nesting: u8,
    pub ratio: u32,
    pub seconds: u32,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            source_bytes: 64 * 1024 * 1024,
            expanded_bytes: 256 * 1024 * 1024,
            member_bytes: 64 * 1024 * 1024,
            members: 1024,
            nesting: 3,
            ratio: 100,
            seconds: 60,
        }
    }
}
impl ArchiveLimits {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_bytes == 0
            || self.source_bytes > 1024 * 1024 * 1024
            || self.expanded_bytes == 0
            || self.expanded_bytes > 4 * 1024 * 1024 * 1024
            || self.member_bytes == 0
            || self.member_bytes > self.expanded_bytes
            || self.members == 0
            || self.members > 10000
            || self.nesting > 8
            || self.ratio == 0
            || self.ratio > 1000
            || self.seconds == 0
            || self.seconds > 300
        {
            return Err("invalid-archive-limits".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diz {
    pub original: Vec<u8>,
    pub encoding: String,
    pub suggestion: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inspection {
    pub media: MediaType,
    pub archive_error: Option<String>,
    pub expanded_bytes: u64,
    pub members: u32,
    pub diz: Option<Diz>,
    pub scan: ScanReport,
}

pub fn detect(bytes: &[u8]) -> MediaType {
    if bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
    {
        MediaType::Zip
    } else if bytes.starts_with(b"7z\xbc\xaf\x27\x1c") {
        MediaType::SevenZip
    } else if bytes.starts_with(b"Rar!\x1a\x07") {
        MediaType::Rar
    } else if bytes.starts_with(b"\x1f\x8b") {
        MediaType::Gzip
    } else if bytes.starts_with(b"BZh") {
        MediaType::Bzip2
    } else if bytes.starts_with(b"\xfd7zXZ\0") {
        MediaType::Xz
    } else if bytes.starts_with(b"\x28\xb5\x2f\xfd") {
        MediaType::Zstandard
    } else if bytes.starts_with(b"\x60\xea") {
        MediaType::Arj
    } else if bytes.len() > 6 && &bytes[2..5] == b"-lh" {
        MediaType::Lha
    } else if bytes.starts_with(&[0x1a]) && bytes.get(1).is_some_and(|v| *v <= 9) {
        MediaType::Arc
    } else if bytes.len() >= 512 && (&bytes[257..262] == b"ustar" || tar_checksum(&bytes[..512])) {
        MediaType::Tar
    } else if bytes.starts_with(b"MZ")
        || bytes.starts_with(b"\x7fELF")
        || bytes.starts_with(b"\xcf\xfa\xed\xfe")
    {
        MediaType::Executable
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        MediaType::Png
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        MediaType::Jpeg
    } else if bytes.starts_with(b"%PDF-") {
        MediaType::Pdf
    } else if bytes
        .iter()
        .all(|b| *b >= 32 || matches!(*b, b'\n' | b'\r' | b'\t'))
        && std::str::from_utf8(bytes).is_ok()
    {
        MediaType::Text
    } else {
        MediaType::Binary
    }
}
fn tar_checksum(bytes: &[u8]) -> bool {
    let value = std::str::from_utf8(&bytes[148..156])
        .ok()
        .and_then(|s| u64::from_str_radix(s.trim_matches([' ', '\0']), 8).ok());
    value.is_some_and(|v| {
        v > 0
            && v == bytes
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if (148..156).contains(&i) {
                        32
                    } else {
                        *b as u64
                    }
                })
                .sum::<u64>()
    })
}

pub fn inspect(
    file: &mut File,
    filename: &str,
    limits: &ArchiveLimits,
    scanner: Option<&dyn Scanner>,
) -> Inspection {
    let mut state = Inspector {
        limits,
        scanner,
        deadline: Instant::now() + Duration::from_secs(limits.seconds.into()),
        expanded: 0,
        members: 0,
        source_size: file.metadata().map(|m| m.len()).unwrap_or(0),
        diz: None,
        root_diz_depth: if sniff(file) == Ok(MediaType::Gzip) {
            1
        } else {
            0
        },
        scan: ScanReport::new("policy", ScanResult::SkippedByPolicy),
    };
    let media = sniff(file).unwrap_or(MediaType::Binary);
    let result = limits.validate().and_then(|()| {
        if state.source_size > limits.source_bytes {
            return Err("source-limit".into());
        }
        state.visit(file, filename, 0)
    });
    Inspection {
        media,
        archive_error: result.err(),
        expanded_bytes: state.expanded,
        members: state.members,
        diz: state.diz,
        scan: state.scan,
    }
}
fn sniff(file: &mut File) -> Result<MediaType, String> {
    file.rewind().map_err(|_| "read-error")?;
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).map_err(|_| "read-error")?;
    file.rewind().map_err(|_| "read-error")?;
    Ok(detect(&buf[..n]))
}
struct Inspector<'a> {
    limits: &'a ArchiveLimits,
    scanner: Option<&'a dyn Scanner>,
    deadline: Instant,
    expanded: u64,
    members: u32,
    source_size: u64,
    diz: Option<Diz>,
    root_diz_depth: u8,
    scan: ScanReport,
}
impl Inspector<'_> {
    fn check(&self) -> Result<(), String> {
        if Instant::now() > self.deadline {
            Err("inspection-timeout".into())
        } else {
            Ok(())
        }
    }
    fn scan(&mut self, file: &mut File) -> Result<(), String> {
        self.check()?;
        if let Some(scanner) = self.scanner {
            file.rewind().map_err(|_| "read-error")?;
            let mut report = scanner.scan(file);
            report.provider = super::scanner::safe(&report.provider);
            report.engine = report.engine.map(|v| super::scanner::safe(&v));
            report.signatures = report.signatures.map(|v| super::scanner::safe(&v));
            report.detection = report.detection.map(|v| super::scanner::safe(&v));
            if severity(&report.result) >= severity(&self.scan.result) {
                self.scan = report;
            }
        }
        file.rewind().map_err(|_| "read-error")?;
        self.check()
    }
    fn member(&mut self, reader: &mut dyn Read) -> Result<File, String> {
        self.members += 1;
        if self.members > self.limits.members {
            return Err("member-count-limit".into());
        }
        let mut file = tempfile::tempfile().map_err(|_| "staging-error")?;
        let mut buffer = [0; 65536];
        let mut size = 0u64;
        loop {
            self.check()?;
            let n = reader.read(&mut buffer).map_err(|_| "malformed-archive")?;
            if n == 0 {
                break;
            }
            size += n as u64;
            self.expanded += n as u64;
            if size > self.limits.member_bytes
                || self.expanded > self.limits.expanded_bytes
                || self.expanded
                    > self
                        .source_size
                        .max(1024)
                        .saturating_mul(self.limits.ratio.into())
            {
                return Err("expansion-limit".into());
            }
            file.write_all(&buffer[..n]).map_err(|_| "staging-error")?;
        }
        file.rewind().map_err(|_| "staging-error")?;
        Ok(file)
    }
    fn visit(&mut self, file: &mut File, name: &str, depth: u8) -> Result<(), String> {
        self.scan(file)?;
        let media = sniff(file)?;
        if media.unsupported() {
            return Err("unsupported-archive".into());
        }
        if !matches!(media, MediaType::Zip | MediaType::Tar | MediaType::Gzip) {
            if archive_extension(name) {
                return Err("archive-signature-mismatch".into());
            }
            return Ok(());
        }
        if depth > self.limits.nesting {
            return Err("nesting-limit".into());
        }
        let mut names = BTreeMap::new();
        match media {
            MediaType::Zip => {
                let declared = crate::file_maintenance::declared_standard_zip_member_count(file)
                    .map_err(|_| "malformed-archive")?
                    .ok_or("unsupported-zip-directory")?;
                if declared > self.limits.members as usize {
                    return Err("member-count-limit".into());
                }
                let mut zip = zip::ZipArchive::new(file).map_err(|_| "malformed-archive")?;
                if zip.len() != declared {
                    return Err("inconsistent-zip-directory".into());
                }
                if zip
                    .has_overlapping_files()
                    .map_err(|_| "malformed-archive")?
                {
                    return Err("overlapping-archive-members".into());
                }
                if zip.len() > self.limits.members as usize {
                    return Err("member-count-limit".into());
                }
                for i in 0..zip.len() {
                    self.check()?;
                    if zip
                        .by_index_raw(i)
                        .map_err(|_| "malformed-archive")?
                        .encrypted()
                    {
                        return Err("encrypted-archive".into());
                    }
                    let mut member = zip
                        .by_index(i)
                        .map_err(|_| "encrypted-or-malformed-archive")?;
                    if member.encrypted() {
                        return Err("encrypted-archive".into());
                    }
                    let member_name = member.name().to_owned();
                    let directory = member.is_dir();
                    path(&member_name, directory, &mut names)?;
                    if let Some(mode) = member.unix_mode() {
                        let kind = mode & 0o170000;
                        if kind != 0 && kind != 0o100000 && !(directory && kind == 0o040000) {
                            return Err("archive-link-or-special".into());
                        }
                    }
                    if directory {
                        self.members += 1;
                        if self.members > self.limits.members {
                            return Err("member-count-limit".into());
                        }
                        continue;
                    }
                    if member.size() > self.limits.member_bytes {
                        return Err("member-size-limit".into());
                    }
                    let mut extracted = self.member(&mut member)?;
                    self.description(&mut extracted, &member_name, depth)?;
                    self.visit(&mut extracted, &member_name, depth + 1)?;
                }
            }
            MediaType::Tar => {
                let mut tar = tar::Archive::new(file);
                // Raw entries retain headers that could otherwise alter path interpretation.
                for entry in tar.entries().map_err(|_| "malformed-archive")?.raw(true) {
                    self.check()?;
                    let mut entry = entry.map_err(|_| "malformed-archive")?;
                    let member_name = std::str::from_utf8(&entry.path_bytes())
                        .map_err(|_| "invalid-member-name")?
                        .to_owned();
                    let kind = entry.header().entry_type();
                    if !kind.is_file() && !kind.is_dir() {
                        return Err("archive-link-or-special".into());
                    }
                    path(&member_name, kind.is_dir(), &mut names)?;
                    if kind.is_dir() {
                        self.members += 1;
                        if self.members > self.limits.members {
                            return Err("member-count-limit".into());
                        }
                        continue;
                    }
                    if entry.size() > self.limits.member_bytes {
                        return Err("member-size-limit".into());
                    }
                    let mut extracted = self.member(&mut entry)?;
                    self.description(&mut extracted, &member_name, depth)?;
                    self.visit(&mut extracted, &member_name, depth + 1)?;
                }
            }
            MediaType::Gzip => {
                let mut decoder = flate2::read::MultiGzDecoder::new(file);
                let mut expanded = self.member(&mut decoder)?;
                // The gzip header's filename is never an extraction path.
                self.visit(&mut expanded, "gzip-member", depth + 1)?;
            }
            _ => unreachable!(),
        }
        self.check()
    }
    fn description(&mut self, file: &mut File, name: &str, depth: u8) -> Result<(), String> {
        // Gzip wrapping a TAR is one container layer, but only its root entries count.
        if depth != self.root_diz_depth || !name.eq_ignore_ascii_case("FILE_ID.DIZ") {
            return Ok(());
        }
        if self.diz.is_some() {
            return Err("ambiguous-file-id-diz".into());
        }
        let size = file.metadata().map_err(|_| "read-error")?.len();
        if size > 16384 {
            return Err("file-id-diz-limit".into());
        }
        file.rewind().map_err(|_| "read-error")?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|_| "read-error")?;
        if bytes.contains(&0) {
            return Err("invalid-file-id-diz".into());
        }
        let (text, encoding) = match std::str::from_utf8(&bytes) {
            Ok(s) => (s.to_owned(), "utf-8"),
            Err(_) => (crate::file_maintenance::decode_cp437(&bytes), "cp437"),
        };
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let suggestion: String = normalized
            .chars()
            .filter(|c| !c.is_control() || matches!(*c, '\n' | '\t'))
            .take(4096)
            .collect();
        self.diz = Some(Diz {
            original: bytes,
            encoding: encoding.into(),
            suggestion,
        });
        Ok(())
    }
}
fn severity(result: &ScanResult) -> u8 {
    match result {
        ScanResult::SkippedByPolicy => 0,
        ScanResult::Clean => 1,
        ScanResult::Unsupported => 2,
        ScanResult::ScannerUnavailable => 3,
        ScanResult::ScannerError => 4,
        ScanResult::Suspicious => 5,
        ScanResult::MalwareDetected => 6,
    }
}
fn archive_extension(name: &str) -> bool {
    name.rsplit('.').next().is_some_and(|e| {
        [
            "zip", "tar", "tgz", "gz", "7z", "rar", "bz2", "xz", "zst", "arc", "arj", "lzh", "lha",
        ]
        .contains(&e.to_ascii_lowercase().as_str())
    })
}
fn path(name: &str, directory: bool, seen: &mut BTreeMap<String, bool>) -> Result<(), String> {
    if name.len() > 1024
        || name.starts_with('/')
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err("unsafe-member-path".into());
    }
    let name = name.trim_end_matches('/');
    if name.is_empty()
        || name
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("unsafe-member-path".into());
    }
    let key = name.to_lowercase();
    if seen.contains_key(&key)
        || seen.iter().any(|(p, d)| {
            (!*d && key.starts_with(&format!("{p}/")))
                || (!directory && p.starts_with(&format!("{key}/")))
        })
    {
        return Err("conflicting-member-path".into());
    }
    seen.insert(key, directory);
    Ok(())
}

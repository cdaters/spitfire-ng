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

//! Stream-oriented scanner providers. Provider errors never mean clean.
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanResult {
    Clean,
    MalwareDetected,
    Suspicious,
    ScannerUnavailable,
    ScannerError,
    Unsupported,
    SkippedByPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanReport {
    pub provider: String,
    pub result: ScanResult,
    pub engine: Option<String>,
    pub signatures: Option<String>,
    pub detection: Option<String>,
    pub timestamp: i64,
}
impl ScanReport {
    pub fn new(provider: &str, result: ScanResult) -> Self {
        Self {
            provider: safe(provider),
            result,
            engine: None,
            signatures: None,
            detection: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// Implementations must bound I/O, avoid modifying input, and return safe metadata.
pub trait Scanner {
    fn scan(&self, source: &mut dyn Read) -> ScanReport;
}

pub struct NoScanner;
impl Scanner for NoScanner {
    fn scan(&self, _source: &mut dyn Read) -> ScanReport {
        ScanReport::new("none", ScanResult::ScannerUnavailable)
    }
}

/// Explicit local ClamD service. No shell, filename or environment is transmitted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clamd {
    pub address: SocketAddr,
}
impl Scanner for Clamd {
    fn scan(&self, source: &mut dyn Read) -> ScanReport {
        if !self.address.ip().is_loopback() {
            return outcome(Err(std::io::ErrorKind::NotConnected.into()));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let result = (|| -> std::io::Result<String> {
            let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(3))?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            exchange(source, &mut stream, deadline)
        })();
        outcome(result)
    }
}

fn exchange<S: Read + Write>(
    source: &mut dyn Read,
    stream: &mut S,
    deadline: std::time::Instant,
) -> std::io::Result<String> {
    stream.write_all(b"zINSTREAM\0")?;
    let mut buffer = [0u8; 65536];
    loop {
        if std::time::Instant::now() >= deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let n = source.read(&mut buffer)?;
        stream.write_all(&(n as u32).to_be_bytes())?;
        if n == 0 {
            break;
        }
        stream.write_all(&buffer[..n])?;
    }
    let mut response = Vec::new();
    for _ in 0..1024 {
        if std::time::Instant::now() >= deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return String::from_utf8(response).map_err(|_| std::io::ErrorKind::InvalidData.into());
        }
        response.push(byte[0]);
    }
    Err(std::io::ErrorKind::InvalidData.into())
}
fn outcome(result: std::io::Result<String>) -> ScanReport {
    let mut report = ScanReport::new("clamd", ScanResult::ScannerUnavailable);
    match result {
        Ok(text) if text == "stream: OK" => report.result = ScanResult::Clean,
        Ok(text) if text.starts_with("stream: ") && text.ends_with(" FOUND") => {
            report.result = ScanResult::MalwareDetected;
            report.detection = Some(safe(
                text.trim_start_matches("stream: ")
                    .trim_end_matches(" FOUND"),
            ));
        }
        Ok(_) => report.result = ScanResult::ScannerError,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::NotConnected
            ) => {}
        Err(_) => report.result = ScanResult::ScannerError,
    }
    report
}

pub(crate) fn safe(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && *c != '\u{1b}')
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Stream {
        response: std::io::Cursor<Vec<u8>>,
        sent: Vec<u8>,
    }
    impl Read for Stream {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.response.read(bytes)
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.sent.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn clamd_stream_contract_distinguishes_clean_detection_and_error() {
        for (reply, expected) in [
            (b"stream: OK\0".as_slice(), ScanResult::Clean),
            (
                b"stream: Synthetic.Mock FOUND\0".as_slice(),
                ScanResult::MalwareDetected,
            ),
            (
                b"INSTREAM size limit exceeded. ERROR\0".as_slice(),
                ScanResult::ScannerError,
            ),
            (b"malformed".as_slice(), ScanResult::ScannerError),
        ] {
            let mut stream = Stream {
                response: std::io::Cursor::new(reply.to_vec()),
                sent: vec![],
            };
            let result = exchange(
                &mut std::io::Cursor::new(b"harmless fixture"),
                &mut stream,
                std::time::Instant::now() + Duration::from_secs(1),
            );
            assert_eq!(outcome(result).result, expected);
            let mut expected = b"zINSTREAM\0".to_vec();
            expected.extend_from_slice(&16u32.to_be_bytes());
            expected.extend_from_slice(b"harmless fixture");
            expected.extend_from_slice(&0u32.to_be_bytes());
            assert_eq!(stream.sent, expected);
        }
        let mut oversized = Stream {
            response: std::io::Cursor::new(vec![b'x'; 1025]),
            sent: vec![],
        };
        assert_eq!(
            outcome(exchange(
                &mut std::io::empty(),
                &mut oversized,
                std::time::Instant::now() + Duration::from_secs(1)
            ))
            .result,
            ScanResult::ScannerError
        );
        assert_eq!(
            Clamd {
                address: "192.0.2.1:3310".parse().unwrap()
            }
            .scan(&mut std::io::Cursor::new(b"no network call"))
            .result,
            ScanResult::ScannerUnavailable
        );
        assert_eq!(
            outcome(Err(std::io::ErrorKind::TimedOut.into())).result,
            ScanResult::ScannerUnavailable
        );
    }
}

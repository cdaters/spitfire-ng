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

use std::error::Error;
use std::fmt;

use crate::parse_pascal_short_string;

pub const HELP_RECORD_COUNT: usize = 55;
pub const HELP_LINE_CAPACITY: usize = 60;
pub const HELP_RECORD_SIZE: usize = 6 * (HELP_LINE_CAPACITY + 1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpRecord {
    fields: [[u8; HELP_LINE_CAPACITY + 1]; 6],
}

impl HelpRecord {
    pub fn from_lines(lines: [&[u8]; 6]) -> Result<Self, HelpError> {
        let mut fields = [[0_u8; HELP_LINE_CAPACITY + 1]; 6];
        for (index, line) in lines.into_iter().enumerate() {
            if line.len() > HELP_LINE_CAPACITY {
                return Err(HelpError::LineTooLong {
                    record: 0,
                    line: index + 1,
                    actual: line.len(),
                    maximum: HELP_LINE_CAPACITY,
                });
            }
            fields[index][0] = line.len() as u8;
            fields[index][1..=line.len()].copy_from_slice(line);
        }
        Ok(Self { fields })
    }

    pub fn line(&self, index: usize) -> Option<&[u8]> {
        let field = self.fields.get(index)?;
        parse_pascal_short_string(field, HELP_LINE_CAPACITY)
            .ok()
            .map(|line| line.value_bytes())
    }

    fn parse(input: &[u8], record_number: usize) -> Result<Self, HelpError> {
        let mut fields = [[0_u8; HELP_LINE_CAPACITY + 1]; 6];
        for (line_index, field) in fields.iter_mut().enumerate() {
            let start = line_index * (HELP_LINE_CAPACITY + 1);
            let end = start + HELP_LINE_CAPACITY + 1;
            field.copy_from_slice(&input[start..end]);
            parse_pascal_short_string(field, HELP_LINE_CAPACITY).map_err(|_| {
                HelpError::InvalidLength {
                    record: record_number,
                    line: line_index + 1,
                    length: usize::from(field[0]),
                }
            })?;
        }
        Ok(Self { fields })
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        for field in &self.fields {
            output.extend_from_slice(field);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpFile {
    records: Vec<HelpRecord>,
}

impl HelpFile {
    pub fn new(records: Vec<HelpRecord>) -> Result<Self, HelpError> {
        if records.len() > HELP_RECORD_COUNT {
            return Err(HelpError::TooManyRecords(records.len()));
        }
        Ok(Self { records })
    }

    pub fn parse(input: &[u8]) -> Result<Self, HelpError> {
        if !input.len().is_multiple_of(HELP_RECORD_SIZE) {
            return Err(HelpError::InvalidFileSize(input.len()));
        }
        let count = input.len() / HELP_RECORD_SIZE;
        if count > HELP_RECORD_COUNT {
            return Err(HelpError::TooManyRecords(count));
        }
        let mut records = Vec::with_capacity(count);
        let (record_bytes, remainder) = input.as_chunks::<HELP_RECORD_SIZE>();
        debug_assert!(remainder.is_empty());
        for (index, bytes) in record_bytes.iter().enumerate() {
            records.push(HelpRecord::parse(bytes, index + 1)?);
        }
        Ok(Self { records })
    }

    /// SPITFIRE help record numbers are one-based.
    pub fn record(&self, number: usize) -> Option<&HelpRecord> {
        number
            .checked_sub(1)
            .and_then(|index| self.records.get(index))
    }

    pub fn records(&self) -> &[HelpRecord] {
        &self.records
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.records.len() * HELP_RECORD_SIZE);
        for record in &self.records {
            record.encode_into(&mut output);
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HelpError {
    InvalidFileSize(usize),
    TooManyRecords(usize),
    InvalidLength {
        record: usize,
        line: usize,
        length: usize,
    },
    LineTooLong {
        record: usize,
        line: usize,
        actual: usize,
        maximum: usize,
    },
}

impl fmt::Display for HelpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFileSize(size) => write!(
                formatter,
                "SPITFIRE.HLP size {size} is not a multiple of {HELP_RECORD_SIZE}"
            ),
            Self::TooManyRecords(count) => write!(
                formatter,
                "SPITFIRE.HLP contains {count} records; documented maximum is {HELP_RECORD_COUNT}"
            ),
            Self::InvalidLength {
                record,
                line,
                length,
            } => write!(
                formatter,
                "SPITFIRE.HLP record {record} line {line} has impossible length {length}"
            ),
            Self::LineTooLong {
                record,
                line,
                actual,
                maximum,
            } => write!(
                formatter,
                "help record {record} line {line} is {actual} bytes; maximum is {maximum}"
            ),
        }
    }
}

impl Error for HelpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_round_trips_fixed_pascal_records_with_stale_storage() {
        let mut bytes = vec![0_u8; HELP_RECORD_SIZE];
        bytes[0] = 3;
        bytes[1..4].copy_from_slice(b"ABC");
        bytes[4] = 0xB3;
        let help = HelpFile::parse(&bytes).unwrap();
        assert_eq!(help.record(1).unwrap().line(0).unwrap(), b"ABC");
        assert_eq!(help.to_bytes(), bytes);
        assert!(help.record(0).is_none());
    }

    #[test]
    fn rejects_bad_size_lengths_and_record_count() {
        assert!(matches!(
            HelpFile::parse(&[0; 10]),
            Err(HelpError::InvalidFileSize(10))
        ));
        let mut bad = vec![0_u8; HELP_RECORD_SIZE];
        bad[0] = 61;
        assert!(matches!(
            HelpFile::parse(&bad),
            Err(HelpError::InvalidLength { .. })
        ));
        assert!(matches!(
            HelpFile::parse(&vec![0; HELP_RECORD_SIZE * 56]),
            Err(HelpError::TooManyRecords(56))
        ));
    }
}

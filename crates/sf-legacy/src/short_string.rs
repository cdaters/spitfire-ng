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

use crate::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PascalShortString<'a> {
    field: &'a [u8],
    length: usize,
}

impl<'a> PascalShortString<'a> {
    #[must_use]
    pub fn value_bytes(&self) -> &'a [u8] {
        &self.field[1..1 + self.length]
    }

    #[must_use]
    pub fn storage_bytes(&self) -> &'a [u8] {
        &self.field[1..]
    }

    #[must_use]
    pub fn stale_bytes(&self) -> &'a [u8] {
        &self.field[self.length + 1..]
    }

    #[must_use]
    pub const fn field_bytes(&self) -> &'a [u8] {
        self.field
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub fn parse_pascal_short_string(
    field: &[u8],
    storage_capacity: usize,
) -> Result<PascalShortString<'_>, ParseError> {
    let expected = storage_capacity
        .checked_add(1)
        .ok_or(ParseError::ArithmeticOverflow)?;
    if field.len() != expected {
        return Err(ParseError::InvalidFieldSize {
            expected,
            actual: field.len(),
        });
    }

    let length = usize::from(field[0]);
    if length > storage_capacity {
        return Err(ParseError::InvalidShortStringLength {
            offset: 0,
            length,
            storage_capacity,
        });
    }

    Ok(PascalShortString { field, length })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_short_string_and_stale_storage() {
        let field = [3, b'A', b'B', b'C', b'X', b'Y'];
        let parsed = parse_pascal_short_string(&field, 5).unwrap();
        assert_eq!(parsed.value_bytes(), b"ABC");
        assert_eq!(parsed.storage_bytes(), b"ABCXY");
        assert_eq!(parsed.stale_bytes(), b"XY");
    }

    #[test]
    fn parses_zero_length_short_string() {
        let field = [0, b'X', b'Y'];
        let parsed = parse_pascal_short_string(&field, 2).unwrap();
        assert!(parsed.is_empty());
        assert_eq!(parsed.stale_bytes(), b"XY");
    }

    #[test]
    fn parses_maximum_length_short_string() {
        let field = [3, b'A', b'B', b'C'];
        let parsed = parse_pascal_short_string(&field, 3).unwrap();
        assert_eq!(parsed.value_bytes(), b"ABC");
        assert!(parsed.stale_bytes().is_empty());
    }

    #[test]
    fn rejects_invalid_length() {
        let field = [4, b'A', b'B', b'C'];
        assert_eq!(
            parse_pascal_short_string(&field, 3),
            Err(ParseError::InvalidShortStringLength {
                offset: 0,
                length: 4,
                storage_capacity: 3,
            })
        );
    }

    #[test]
    fn rejects_truncated_field() {
        let field = [1, b'A'];
        assert_eq!(
            parse_pascal_short_string(&field, 2),
            Err(ParseError::InvalidFieldSize {
                expected: 3,
                actual: 2,
            })
        );
    }
}

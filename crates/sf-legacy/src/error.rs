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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    InvalidFieldSize {
        expected: usize,
        actual: usize,
    },
    InvalidShortStringLength {
        offset: usize,
        length: usize,
        storage_capacity: usize,
    },
    ArithmeticOverflow,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof {
                offset,
                needed,
                remaining,
            } => write!(
                f,
                "unexpected end of input at offset 0x{offset:X}: needed {needed} bytes, {remaining} remain"
            ),
            Self::InvalidFieldSize { expected, actual } => write!(
                f,
                "invalid fixed field size: expected {expected} bytes, received {actual}"
            ),
            Self::InvalidShortStringLength {
                offset,
                length,
                storage_capacity,
            } => write!(
                f,
                "invalid Turbo Pascal short-string length {length} at offset 0x{offset:X}; storage capacity is {storage_capacity}"
            ),
            Self::ArithmeticOverflow => write!(f, "size calculation overflow"),
        }
    }
}

impl Error for ParseError {}

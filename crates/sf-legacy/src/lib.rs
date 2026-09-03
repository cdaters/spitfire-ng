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

//! Bounds-checked helpers for documented Turbo Pascal and DOS-era data.

mod error;
mod help;
mod menu;
mod reader;
mod short_string;

pub use error::ParseError;
pub use help::{
    HelpError, HelpFile, HelpRecord, HELP_LINE_CAPACITY, HELP_RECORD_COUNT, HELP_RECORD_SIZE,
};
pub use menu::{MenuEntry, MenuError, MenuFile};
pub use reader::Reader;
pub use short_string::{parse_pascal_short_string, PascalShortString};

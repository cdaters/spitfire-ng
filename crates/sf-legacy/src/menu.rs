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

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

const MAX_MENU_BYTES: usize = 64 * 1024;
const MAX_MENU_LINE_BYTES: usize = 40;

/// One documented SPITFIRE 3.7 menu line:
/// command,description,reserved,security,internal identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuEntry {
    command: u8,
    description: Vec<u8>,
    reserved: Vec<u8>,
    required_security: u16,
    identifier: u8,
}

impl MenuEntry {
    pub fn new(
        command: u8,
        description: Vec<u8>,
        reserved: Vec<u8>,
        required_security: u16,
        identifier: u8,
    ) -> Result<Self, MenuError> {
        validate_single_field("command", &[command])?;
        validate_single_field("identifier", &[identifier])?;
        validate_field("description", &description)?;
        validate_field("reserved", &reserved)?;
        let entry = Self {
            command: command.to_ascii_uppercase(),
            description,
            reserved,
            required_security,
            identifier,
        };
        if entry.encoded_len() > MAX_MENU_LINE_BYTES {
            return Err(MenuError::LineTooLong {
                line: 0,
                actual: entry.encoded_len(),
                maximum: MAX_MENU_LINE_BYTES,
            });
        }
        Ok(entry)
    }

    pub const fn command(&self) -> u8 {
        self.command
    }

    pub fn description(&self) -> &[u8] {
        &self.description
    }

    pub fn reserved(&self) -> &[u8] {
        &self.reserved
    }

    pub const fn required_security(&self) -> u16 {
        self.required_security
    }

    pub const fn identifier(&self) -> u8 {
        self.identifier
    }

    fn encoded_len(&self) -> usize {
        1 + 1
            + self.description.len()
            + 1
            + self.reserved.len()
            + 1
            + self.required_security.to_string().len()
            + 1
            + 1
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        output.push(self.command);
        output.push(b',');
        output.extend_from_slice(&self.description);
        output.push(b',');
        output.extend_from_slice(&self.reserved);
        output.push(b',');
        output.extend_from_slice(self.required_security.to_string().as_bytes());
        output.push(b',');
        output.push(self.identifier);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFile {
    entries: Vec<MenuEntry>,
    dos_eof: bool,
}

impl MenuFile {
    pub fn new(entries: Vec<MenuEntry>) -> Result<Self, MenuError> {
        validate_unique_commands(&entries)?;
        Ok(Self {
            entries,
            dos_eof: false,
        })
    }

    pub fn parse(input: &[u8]) -> Result<Self, MenuError> {
        if input.len() > MAX_MENU_BYTES {
            return Err(MenuError::FileTooLarge {
                actual: input.len(),
                maximum: MAX_MENU_BYTES,
            });
        }

        let dos_eof = input.last() == Some(&0x1A);
        let body = if dos_eof {
            &input[..input.len() - 1]
        } else {
            input
        };
        let mut entries = Vec::new();
        for (index, raw_line) in body.split(|byte| *byte == b'\n').enumerate() {
            let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if line.is_empty() || line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let line_number = index + 1;
            if line.len() > MAX_MENU_LINE_BYTES {
                return Err(MenuError::LineTooLong {
                    line: line_number,
                    actual: line.len(),
                    maximum: MAX_MENU_LINE_BYTES,
                });
            }
            let fields: Vec<&[u8]> = line.split(|byte| *byte == b',').collect();
            if fields.len() != 5 {
                return Err(MenuError::FieldCount {
                    line: line_number,
                    actual: fields.len(),
                });
            }
            validate_single_field_at(line_number, "command", fields[0])?;
            validate_single_field_at(line_number, "identifier", fields[4])?;
            validate_field_at(line_number, "description", fields[1])?;
            validate_field_at(line_number, "reserved", fields[2])?;
            let security_text = std::str::from_utf8(fields[3])
                .map_err(|_| MenuError::InvalidSecurity { line: line_number })?;
            let required_security = security_text
                .parse::<u16>()
                .map_err(|_| MenuError::InvalidSecurity { line: line_number })?;
            entries.push(MenuEntry {
                command: fields[0][0].to_ascii_uppercase(),
                description: fields[1].to_vec(),
                reserved: fields[2].to_vec(),
                required_security,
                identifier: fields[4][0],
            });
        }
        validate_unique_commands(&entries)?;
        Ok(Self { entries, dos_eof })
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    pub fn find_command(&self, command: u8, security: u16) -> Option<&MenuEntry> {
        let command = command.to_ascii_uppercase();
        self.entries
            .iter()
            .find(|entry| entry.command == command && entry.required_security <= security)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for entry in &self.entries {
            entry.encode_into(&mut output);
            output.extend_from_slice(b"\r\n");
        }
        if self.dos_eof {
            output.push(0x1A);
        }
        output
    }
}

fn validate_unique_commands(entries: &[MenuEntry]) -> Result<(), MenuError> {
    let mut commands = HashSet::new();
    for entry in entries {
        if !commands.insert(entry.command) {
            return Err(MenuError::DuplicateCommand(entry.command));
        }
    }
    Ok(())
}

fn validate_single_field(name: &'static str, field: &[u8]) -> Result<(), MenuError> {
    validate_single_field_at(0, name, field)
}

fn validate_single_field_at(
    line: usize,
    name: &'static str,
    field: &[u8],
) -> Result<(), MenuError> {
    if field.len() != 1 || field[0] == b',' || field[0] == 0 || field[0] == 0x1A {
        return Err(MenuError::InvalidField { line, name });
    }
    Ok(())
}

fn validate_field(name: &'static str, field: &[u8]) -> Result<(), MenuError> {
    validate_field_at(0, name, field)
}

fn validate_field_at(line: usize, name: &'static str, field: &[u8]) -> Result<(), MenuError> {
    if field.contains(&b',') || field.contains(&0) || field.contains(&0x1A) {
        return Err(MenuError::InvalidField { line, name });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuError {
    FileTooLarge {
        actual: usize,
        maximum: usize,
    },
    LineTooLong {
        line: usize,
        actual: usize,
        maximum: usize,
    },
    FieldCount {
        line: usize,
        actual: usize,
    },
    InvalidField {
        line: usize,
        name: &'static str,
    },
    InvalidSecurity {
        line: usize,
    },
    DuplicateCommand(u8),
}

impl fmt::Display for MenuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "menu file is {actual} bytes; maximum is {maximum}"
                )
            }
            Self::LineTooLong {
                line,
                actual,
                maximum,
            } => write!(
                formatter,
                "menu line {line} is {actual} bytes; documented maximum is {maximum}"
            ),
            Self::FieldCount { line, actual } => write!(
                formatter,
                "menu line {line} has {actual} comma-separated fields; expected 5"
            ),
            Self::InvalidField { line, name } => {
                write!(formatter, "menu line {line} has an invalid {name} field")
            }
            Self::InvalidSecurity { line } => {
                write!(formatter, "menu line {line} has an invalid security level")
            }
            Self::DuplicateCommand(command) => write!(
                formatter,
                "menu contains duplicate command byte 0x{command:02X}"
            ),
        }
    }
}

impl Error for MenuError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_menu_line_and_round_trips_cp437_bytes() {
        let input = b"M,<M>.......... Message Section,,5,E\r\nX,<X>... \xB3 Toggle,,0,B\r\n\x1A";
        let menu = MenuFile::parse(input).unwrap();
        assert_eq!(menu.entries().len(), 2);
        assert_eq!(menu.entries()[0].identifier(), b'E');
        assert_eq!(menu.entries()[0].required_security(), 5);
        assert_eq!(menu.entries()[1].description()[7], 0xB3);
        assert_eq!(menu.to_bytes(), input);
    }

    #[test]
    fn rejects_malformed_and_duplicate_lines() {
        assert!(matches!(
            MenuFile::parse(b"M,description,5,E\n"),
            Err(MenuError::FieldCount { .. })
        ));
        assert!(matches!(
            MenuFile::parse(b"M,one,,0,E\nM,two,,0,Q\n"),
            Err(MenuError::DuplicateCommand(b'M'))
        ));
        let long = format!("A,{},,0,A\n", "x".repeat(40));
        assert!(matches!(
            MenuFile::parse(long.as_bytes()),
            Err(MenuError::LineTooLong { .. })
        ));
    }
}

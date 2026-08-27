use crate::{parse_pascal_short_string, ParseError, PascalShortString};

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    #[must_use]
    pub const fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.input.len() - self.position
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], ParseError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ParseError::ArithmeticOverflow)?;
        let bytes = self
            .input
            .get(self.position..end)
            .ok_or(ParseError::UnexpectedEof {
                offset: self.position,
                needed: length,
                remaining: self.remaining(),
            })?;
        self.position = end;
        Ok(bytes)
    }

    pub fn read_u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.take(1)?[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16, ParseError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_short_string(
        &mut self,
        storage_capacity: usize,
    ) -> Result<PascalShortString<'a>, ParseError> {
        let offset = self.position;
        let field_size = storage_capacity
            .checked_add(1)
            .ok_or(ParseError::ArithmeticOverflow)?;
        let field = self.take(field_size)?;
        parse_pascal_short_string_at(field, storage_capacity, offset)
    }
}

fn parse_pascal_short_string_at(
    field: &[u8],
    storage_capacity: usize,
    offset: usize,
) -> Result<PascalShortString<'_>, ParseError> {
    parse_pascal_short_string(field, storage_capacity).map_err(|error| match error {
        ParseError::InvalidShortStringLength {
            length,
            storage_capacity,
            ..
        } => ParseError::InvalidShortStringLength {
            offset,
            length,
            storage_capacity,
        },
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_little_endian_u16() {
        let mut reader = Reader::new(&[0xE9, 0x04]);
        assert_eq!(reader.read_u16_le().unwrap(), 1257);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn bounds_checks_reads() {
        let mut reader = Reader::new(&[0xE9]);
        assert_eq!(
            reader.read_u16_le(),
            Err(ParseError::UnexpectedEof {
                offset: 0,
                needed: 2,
                remaining: 1,
            })
        );
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn reports_short_string_offset() {
        let mut reader = Reader::new(&[0x00, 0x02, b'A']);
        reader.read_u8().unwrap();
        assert_eq!(
            reader.read_short_string(1),
            Err(ParseError::InvalidShortStringLength {
                offset: 1,
                length: 2,
                storage_capacity: 1,
            })
        );
    }
}

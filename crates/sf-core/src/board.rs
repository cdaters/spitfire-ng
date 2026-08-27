use thiserror::Error;

const MAX_BOARD_NAME_BYTES: usize = 80;
const MAX_SYSOP_NAME_BYTES: usize = 60;

/// Caller-visible board identity required by the stock startup path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardIdentity {
    name: String,
    sysop_name: String,
}

impl BoardIdentity {
    pub fn new(
        name: impl Into<String>,
        sysop_name: impl Into<String>,
    ) -> Result<Self, BoardIdentityError> {
        let name = validate("board name", name.into(), MAX_BOARD_NAME_BYTES)?;
        let sysop_name = validate("Sysop name", sysop_name.into(), MAX_SYSOP_NAME_BYTES)?;
        Ok(Self { name, sysop_name })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sysop_name(&self) -> &str {
        &self.sysop_name
    }
}

fn validate(
    field: &'static str,
    value: String,
    maximum: usize,
) -> Result<String, BoardIdentityError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BoardIdentityError::Empty { field });
    }
    if trimmed.len() > maximum {
        return Err(BoardIdentityError::TooLong {
            field,
            actual: trimmed.len(),
            maximum,
        });
    }
    Ok(trimmed.to_owned())
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BoardIdentityError {
    #[error("{field} must not be empty")]
    Empty { field: &'static str },
    #[error("{field} is {actual} bytes; maximum is {maximum}")]
    TooLong {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_normalizes_board_identity() {
        let identity = BoardIdentity::new("  Fixture Board  ", " Fixture Sysop ").unwrap();
        assert_eq!(identity.name(), "Fixture Board");
        assert_eq!(identity.sysop_name(), "Fixture Sysop");
    }

    #[test]
    fn rejects_empty_identity_fields() {
        assert_eq!(
            BoardIdentity::new(" ", "Sysop").unwrap_err(),
            BoardIdentityError::Empty {
                field: "board name"
            }
        );
    }
}

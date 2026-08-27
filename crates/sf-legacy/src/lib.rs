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

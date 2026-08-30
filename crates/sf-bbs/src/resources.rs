use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sf_core::{
    DisplayFormat, DisplayResource, DisplaySource, HelpRecord, LogicalPath, LogicalPaths,
    MenuDefinition, MenuItem, MenuSection, NativeThoughtCatalog, StockResources, TerminalInfo,
};
use sf_legacy::{HelpFile, MenuFile};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::ApplicationError;
use crate::{PresentationResolver, ProfileFormat};

const MAX_DISPLAY_BYTES: usize = 1024 * 1024;
const MAX_DISPLAY_FILES: usize = 4096;
pub(crate) const BUILTIN_SYSOP_MENU: &[u8] = b"Q,<Q>........ Quit To Main Menu,,50,C\r\nX,<X>........ Xpert Mode Toggle,,50,B\r\nG,<G>........ Goodbye & Log Off,,50,A\r\n";

pub(crate) fn public_resource_digests(
    paths: &LogicalPaths,
) -> Result<Vec<(&'static str, String)>, ApplicationError> {
    let display = paths.get(LogicalPath::Display);
    let definitions = [
        (
            "bulletins",
            (0..=99)
                .map(|number| {
                    if number == 0 {
                        "BULLETIN.BBS".to_owned()
                    } else {
                        format!("BULLET{number}.BBS")
                    }
                })
                .collect::<Vec<_>>(),
        ),
        ("newsletter", vec!["SFNWSLTR.BBS".to_owned()]),
        ("thoughts", vec!["THOUGHTS.NG".to_owned()]),
    ];
    let mut output = Vec::new();
    for (kind, names) in definitions {
        let mut digest = Sha256::new();
        for name in names {
            if let Some(path) = find_case_insensitive(display, &name) {
                let bytes = read_bounded(&path, MAX_DISPLAY_BYTES)?;
                digest.update(name.as_bytes());
                digest.update((bytes.len() as u64).to_le_bytes());
                digest.update(bytes);
            }
        }
        output.push((kind, format!("{:x}", digest.finalize())));
    }
    Ok(output)
}

pub fn load_stock_resources(
    paths: &LogicalPaths,
    terminal: &TerminalInfo,
    presentation: &PresentationResolver,
) -> Result<StockResources, ApplicationError> {
    let system = paths.get(LogicalPath::System);
    let display = paths.get(LogicalPath::Display);
    let main = load_menu(system, "SFMAIN.MNU", MenuSection::Main)?;
    let message = load_menu(system, "SFMSG.MNU", MenuSection::Message)?;
    let file = load_menu(system, "SFFILE.MNU", MenuSection::File)?;
    let sysop = load_optional_menu(
        system,
        "SFSYSOP.MNU",
        MenuSection::Sysop,
        BUILTIN_SYSOP_MENU,
    )?;
    let help = presentation.help(system);

    let mut menus = BTreeMap::new();
    menus.insert(MenuSection::Main, main);
    menus.insert(MenuSection::Message, message);
    menus.insert(MenuSection::File, file);
    menus.insert(MenuSection::Sysop, sysop);

    let displays = presentation.displays(display, terminal);
    let thoughts = find_case_insensitive(display, "THOUGHTS.NG").and_then(|path| {
        match read_bounded(
            &path,
            sf_core::MAX_NATIVE_THOUGHTS * (sf_core::MAX_NATIVE_THOUGHT_BYTES + 1),
        )
        .and_then(|bytes| NativeThoughtCatalog::parse(&bytes).map_err(ApplicationError::from))
        {
            Ok(catalog) => Some(catalog),
            Err(error) => {
                warn!(path = %path.display(), error = %error, "project-native thought catalog is unavailable; continuing without a thought");
                None
            }
        }
    });

    Ok(StockResources {
        prelogin: display_or(&displays, "SFPRELOG", b"@CLS@SPITFIRE NG\r\n"),
        welcome: display_or(
            &displays,
            "WELCOME1",
            b"Welcome to @BOARD@\r\nNode @NODE@ is ready.\r\n",
        ),
        goodbye: displays.get("GOODBYE").cloned().unwrap_or(DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"Thank you for calling @BOARD@.\r\n".to_vec(),
        }),
        page_off: displays.get("SFPGOFF").cloned().unwrap_or(DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"The Sysop page is currently unavailable.\r\n".to_vec(),
        }),
        page_unanswered: displays.get("SFUNANS").cloned().unwrap_or(DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"The Sysop did not answer your page.\r\n".to_vec(),
        }),
        page_already_requested: displays.get("SFPAGED").cloned().unwrap_or(DisplayResource {
            format: DisplayFormat::Bbs,
            source: DisplaySource::EngineBuiltIn,
            bytes: b"Your page has already been sent to the Sysop.\r\n".to_vec(),
        }),
        chat_caller_initiated: displays
            .get("USERINIT")
            .cloned()
            .unwrap_or(DisplayResource {
                format: DisplayFormat::Bbs,
                source: DisplaySource::EngineBuiltIn,
                bytes: b"The Sysop answered. Interactive chat is now active.\r\n".to_vec(),
            }),
        chat_done: displays
            .get("CHATDONE")
            .cloned()
            .unwrap_or(DisplayResource {
                format: DisplayFormat::Bbs,
                source: DisplaySource::EngineBuiltIn,
                bytes: b"Sysop chat ended. Returning to the BBS.\r\n".to_vec(),
            }),
        menus,
        displays,
        help_records: help,
        thoughts,
    })
}

fn display_or(
    displays: &BTreeMap<String, DisplayResource>,
    stem: &str,
    fallback: &[u8],
) -> DisplayResource {
    displays.get(stem).cloned().unwrap_or(DisplayResource {
        format: DisplayFormat::Bbs,
        source: DisplaySource::EngineBuiltIn,
        bytes: fallback.to_vec(),
    })
}

fn load_menu(
    directory: &Path,
    name: &str,
    section: MenuSection,
) -> Result<MenuDefinition, ApplicationError> {
    let path = find_case_insensitive(directory, name).unwrap_or_else(|| directory.join(name));
    let bytes = read_bounded(&path, 64 * 1024)?;
    parse_menu_bytes(path, &bytes, section)
}

fn load_optional_menu(
    directory: &Path,
    name: &str,
    section: MenuSection,
    fallback: &[u8],
) -> Result<MenuDefinition, ApplicationError> {
    let Some(path) = find_case_insensitive(directory, name) else {
        warn!(
            name,
            "optional native compatibility menu is missing; using bounded built-in fallback"
        );
        return parse_menu_bytes(directory.join(name), fallback, section);
    };
    match read_bounded(&path, 64 * 1024) {
        Ok(bytes) => match parse_menu_bytes(path.clone(), &bytes, section) {
            Ok(menu) => Ok(menu),
            Err(error) => {
                warn!(path = %path.display(), error = %error, "optional native compatibility menu is malformed; using bounded built-in fallback");
                parse_menu_bytes(path, fallback, section)
            }
        },
        Err(error) => {
            warn!(path = %path.display(), error = %error, "optional native compatibility menu is unavailable; using bounded built-in fallback");
            parse_menu_bytes(path, fallback, section)
        }
    }
}

fn parse_menu_bytes(
    path: PathBuf,
    bytes: &[u8],
    section: MenuSection,
) -> Result<MenuDefinition, ApplicationError> {
    let parsed =
        MenuFile::parse(bytes).map_err(|source| ApplicationError::MenuResource { path, source })?;
    Ok(MenuDefinition {
        section,
        items: parsed
            .entries()
            .iter()
            .map(|entry| MenuItem {
                command: entry.command(),
                description: entry.description().to_vec(),
                required_security: entry.required_security(),
                identifier: entry.identifier(),
            })
            .collect(),
    })
}

pub(crate) fn load_help_file(directory: &Path) -> Option<Vec<HelpRecord>> {
    let path = find_case_insensitive(directory, "SPITFIRE.HLP")
        .unwrap_or_else(|| directory.join("SPITFIRE.HLP"));
    let bytes = match read_bounded(
        &path,
        sf_legacy::HELP_RECORD_SIZE * sf_legacy::HELP_RECORD_COUNT,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!(path = %path.display(), error = %error, "SPITFIRE.HLP unavailable; contextual help will use a bounded fallback");
            return None;
        }
    };
    let help = match HelpFile::parse(&bytes) {
        Ok(help) => help,
        Err(error) => {
            warn!(path = %path.display(), error = %error, "SPITFIRE.HLP is malformed; contextual help will use a bounded fallback");
            return None;
        }
    };
    Some(
        help.records()
            .iter()
            .map(|record| HelpRecord {
                lines: std::array::from_fn(|index| record.line(index).unwrap_or_default().to_vec()),
            })
            .collect(),
    )
}

pub(crate) fn load_display_layer(
    directory: &Path,
    terminal: &TerminalInfo,
) -> Result<BTreeMap<String, DisplayResource>, ApplicationError> {
    let mut files = fs::read_dir(directory)
        .map_err(|source| ApplicationError::ReadResource {
            path: directory.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    if files.len() > MAX_DISPLAY_FILES {
        return Err(ApplicationError::ResourceTooLarge {
            path: directory.to_path_buf(),
            actual: files.len(),
            maximum: MAX_DISPLAY_FILES,
        });
    }

    // BBS is loaded first. A usable CLR then replaces it only for an ANSI-
    // capable terminal, matching the documented CLR -> BBS fallback.
    files.sort_by_key(|path| {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        usize::from(extension.eq_ignore_ascii_case("CLR"))
    });
    let mut resources = BTreeMap::new();
    for path in files {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let format = if extension.eq_ignore_ascii_case("BBS") {
            DisplayFormat::Bbs
        } else if extension.eq_ignore_ascii_case("CLR") && terminal.capabilities.ansi {
            DisplayFormat::Clr
        } else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let bytes = match read_bounded(&path, MAX_DISPLAY_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!(path = %path.display(), error = %error, "optional display resource rejected; trying lower-precedence fallback");
                continue;
            }
        };
        if let Err(reason) = validate_display(format, &bytes) {
            warn!(path = %path.display(), reason, "malformed optional display resource rejected; trying lower-precedence fallback");
            continue;
        }
        resources.insert(
            stem.to_ascii_uppercase(),
            DisplayResource {
                format,
                source: DisplaySource::BoardOverride,
                bytes,
            },
        );
    }
    Ok(resources)
}

pub(crate) fn validate_profile_display(
    format: ProfileFormat,
    bytes: &[u8],
) -> Result<(), &'static str> {
    match format {
        ProfileFormat::Bbs => validate_display(DisplayFormat::Bbs, bytes),
        ProfileFormat::Clr => validate_display(DisplayFormat::Clr, bytes),
        ProfileFormat::SpitfireHelp => Err("help data cannot be used as a display"),
    }
}

fn validate_display(format: DisplayFormat, bytes: &[u8]) -> Result<(), &'static str> {
    if format != DisplayFormat::Clr {
        return Ok(());
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            index += 1;
            continue;
        }
        index += 1;
        let Some(next) = bytes.get(index).copied() else {
            return Err("trailing ANSI escape byte");
        };
        if next != b'[' {
            index += 1;
            continue;
        }
        index += 1;
        let start = index;
        loop {
            let Some(byte) = bytes.get(index).copied() else {
                return Err("unterminated ANSI CSI sequence");
            };
            index += 1;
            if (0x40..=0x7e).contains(&byte) {
                break;
            }
            if index - start > 64 {
                return Err("ANSI CSI sequence exceeds 64 bytes");
            }
        }
    }
    Ok(())
}

pub(crate) fn find_case_insensitive(directory: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(directory).ok()?;
    entries.filter_map(Result::ok).find_map(|entry| {
        entry
            .file_name()
            .to_str()
            .filter(|candidate| candidate.eq_ignore_ascii_case(name))
            .map(|_| entry.path())
    })
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, ApplicationError> {
    let metadata = fs::metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ApplicationError::MissingResource(path.to_path_buf())
        } else {
            ApplicationError::ReadResource {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    let actual = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if actual > maximum {
        return Err(ApplicationError::ResourceTooLarge {
            path: path.to_path_buf(),
            actual,
            maximum,
        });
    }
    fs::read(path).map_err(|source| ApplicationError::ReadResource {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{RuntimeConfig, TerminalInfo};

    use crate::initialize_fixture_board;

    fn resolver(paths: &LogicalPaths) -> PresentationResolver {
        PresentationResolver::load(
            paths,
            &RuntimeConfig::synthetic_fixture()
                .validate()
                .unwrap()
                .presentation,
        )
    }

    #[test]
    fn loads_fixture_menus_help_and_ansi_resources() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture");
        initialize_fixture_board(&root).unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths)).unwrap();
        assert_eq!(
            resources
                .menu(MenuSection::Main)
                .unwrap()
                .find(b'M', 10)
                .unwrap()
                .identifier,
            b'E'
        );
        assert_eq!(resources.welcome.format, DisplayFormat::Clr);
        assert_eq!(
            resources
                .menu(MenuSection::Sysop)
                .unwrap()
                .find(b'Q', 50)
                .unwrap()
                .identifier,
            b'C'
        );
        assert_eq!(
            resources
                .menu_display(MenuSection::Main, 10)
                .unwrap()
                .format,
            DisplayFormat::Clr
        );
        assert_eq!(
            resources
                .menu_display(MenuSection::Sysop, 50)
                .unwrap()
                .format,
            DisplayFormat::Clr
        );
        assert!(resources.menu_display(MenuSection::Main, 9).is_none());
        assert!(resources.help_record(21).is_some());
        assert_eq!(
            resources.display("BULLET1").unwrap().source,
            DisplaySource::BoardOverride
        );
        assert_eq!(
            resources.display("SFNWSLTR").unwrap().source,
            DisplaySource::BoardOverride
        );
        assert_eq!(
            sf_core::ThoughtCatalogReader::thoughts(resources.thoughts.as_ref().unwrap()).len(),
            2
        );
    }

    #[test]
    fn missing_or_malformed_sysop_menu_uses_the_bounded_upgrade_fallback() {
        for malformed in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("fixture");
            initialize_fixture_board(&root).unwrap();
            let menu = root.join("system/SFSYSOP.MNU");
            if malformed {
                std::fs::write(&menu, b"not,a,valid,menu").unwrap();
            } else {
                std::fs::remove_file(&menu).unwrap();
            }
            let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let resources =
                load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths))
                    .unwrap();
            let sysop = resources.menu(MenuSection::Sysop).unwrap();
            assert_eq!(sysop.items.len(), 3);
            assert_eq!(sysop.find(b'Q', 50).unwrap().identifier, b'C');
            assert!(sysop.find(b'Q', 49).is_none());
        }
    }

    #[test]
    fn malformed_or_oversized_native_thought_catalog_is_optional_and_bounded() {
        for bytes in [
            vec![0xff],
            vec![b'x'; sf_core::MAX_NATIVE_THOUGHTS * (sf_core::MAX_NATIVE_THOUGHT_BYTES + 1) + 1],
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("fixture");
            initialize_fixture_board(&root).unwrap();
            std::fs::write(root.join("display/THOUGHTS.NG"), bytes).unwrap();
            let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let resources =
                load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths))
                    .unwrap();
            assert!(resources.thoughts.is_none());
        }
    }

    #[test]
    fn malformed_preferred_clr_falls_back_to_bbs_without_crashing() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture");
        initialize_fixture_board(&root).unwrap();
        std::fs::write(
            root.join("display/MAIN10.BBS"),
            b"\r\n>>>>>>>> MAIN MENU <<<<<<<<\r\n",
        )
        .unwrap();
        std::fs::write(
            root.join("display/SOP50.BBS"),
            b"\r\n>>>>>>>> SYSOP MENU <<<<<<<<\r\n",
        )
        .unwrap();
        std::fs::write(root.join("display/MAIN10.CLR"), b"broken\x1b[").unwrap();
        std::fs::write(root.join("display/SOP50.CLR"), b"broken\x1b[").unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths)).unwrap();
        let display = resources.menu_display(MenuSection::Main, 10).unwrap();
        assert_eq!(display.format, DisplayFormat::Bbs);
        assert!(display.bytes.starts_with(b"\r\n>>>>>>>> MAIN MENU"));
        let sysop = resources.menu_display(MenuSection::Sysop, 50).unwrap();
        assert_eq!(sysop.format, DisplayFormat::Bbs);
        assert!(sysop.bytes.starts_with(b"\r\n>>>>>>>> SYSOP MENU"));
    }

    #[test]
    fn missing_or_corrupt_board_help_uses_profile_fallback() {
        for malformed in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("fixture");
            initialize_fixture_board(&root).unwrap();
            let help = root.join("system/SPITFIRE.HLP");
            if malformed {
                std::fs::write(&help, b"truncated").unwrap();
            }
            let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
            let paths = LogicalPaths::resolve(&root, &validated).unwrap();
            let resources =
                load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths))
                    .unwrap();
            assert!(resources.help_record(21).is_some());
        }
    }

    #[test]
    fn missing_welcome_resources_use_a_safe_synthetic_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture");
        initialize_fixture_board(&root).unwrap();
        std::fs::remove_file(
            root.join("system/presentation-profiles/modern-ng/resources/display/WELCOME1.CLR"),
        )
        .unwrap();
        std::fs::remove_file(
            root.join("system/presentation-profiles/modern-ng/resources/display/WELCOME1.BBS"),
        )
        .unwrap();
        let validated = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(&root, &validated).unwrap();
        let resources =
            load_stock_resources(&paths, &TerminalInfo::in_memory(), &resolver(&paths)).unwrap();
        assert_eq!(resources.welcome.format, DisplayFormat::Bbs);
        assert!(resources
            .welcome
            .bytes
            .windows(7)
            .any(|part| part == b"@BOARD@"));
    }
}

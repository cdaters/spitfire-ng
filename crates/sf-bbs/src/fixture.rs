use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use sf_core::{
    ConferenceAccessMode, ConferenceDefinition, FileAccessMode, FileArea, FileAreaDefinition,
    FileStorage, LogicalPath, LogicalPaths, RuntimeConfig, RuntimeDatabase,
};
use sf_legacy::{HelpFile, HelpRecord};

use crate::presentation::{
    hex_digest, profile_root, EngineCompatibility, FallbackPolicy, ProfileDescriptor,
    ProfileFormat, ProfileResourceKind, ProfileResourceRecord, ProvenanceKind, ProvenanceRecord,
    Redistribution, CLASSIC_PROFILE_ID, CLASSIC_PROFILE_VERSION, MINIMAL_PROFILE_ID,
    MODERN_PROFILE_ID, PROFILE_DESCRIPTOR, PROFILE_FORMAT_VERSION, RESOURCE_API_VERSION,
};
use crate::ApplicationError;

pub const FIXTURE_CONFIG_FILE: &str = "spitfire.toml";
const DISPLAY_NOTICE_FILE: &str = "GENERATED-RESOURCES.txt";
const PROJECT_ASSET_LICENSE: &[u8] = b"SPDX-License-Identifier: MIT OR Apache-2.0\n\nCopyright (C) 2026 Craig Daters and SPITFIRE NG contributors.\nThese project-authored resources are available under the SPITFIRE NG repository's LICENSE-MIT or LICENSE-APACHE terms, at your option.\nHistorical and third-party research material is excluded and retains its original copyright and license.\n";

const MAIN_MENU: &[u8] = b"M,<M>.......... Message Section,,5,E\r\nC,<C>.......... Comment To Sysop,,5,J\r\nF,<F>............. File Section,,5,Q\r\nP,<P>............ Page The Sysop,,5,H\r\nY,<Y>.......... Your Statistics,,5,G\r\nR,<R>....... Your Caller Profile,,5,D\r\nU,<U>.... Terminal Preferences,,5,R\r\nV,<V>........... About SPITFIRE NG,,5,V\r\nB,<B>................ Bulletins,,5,Y\r\n#,<#>......... Caller Directory,,5,L\r\nL,<L>......... Locate A Caller,,5,I\r\nT,<T>...... System Information,,5,K\r\nN,<N>............... Newsletter,,5,X\r\nO,<O>..... Other BBS Information,,5,P\r\nA,<A>............ Add Other BBS,,5,C\r\n@,<@>.......... Sysop Utilities,,50,F\r\nX,<X>........ Xpert Mode Toggle,,5,B\r\nG,<G>........ Goodbye & Log Off,,5,A\r\n?,<?>....... HELP With Commands,,5,?\r\n";
const MESSAGE_MENU: &[u8] = b"C,<C>....... Change Conference,,5,Z\r\nR,<R>............. Read Messages,,5,I\r\nB,<B>........... Browse Messages,,5,J\r\nE,<E>......... Enter New Message,,5,L\r\nY,<Y>............. Your Messages,,5,G\r\nA,<A>.... Alter Conference Queue,,5,K\r\nS,<S>.. Specific Caller Messages,,5,S\r\nT,<T>............... Text Search,,5,X\r\nF,<F>.............. File Section,,5,D\r\nQ,<Q>......... Quit To Main Menu,,5,C\r\n@,<@>........... Sysop Utilities,,50,R\r\nX,<X>......... Xpert Mode Toggle,,5,B\r\nG,<G>......... Goodbye & Log Off,,5,A\r\n?,<?>........ HELP With Commands,,5,?\r\n";
const FILE_MENU: &[u8] = b"C,<C>......... Change File Area,,5,Z\r\nL,<L>.. List Files In This Area,,5,X\r\nR,<R>......... Read A Text File,,5,J\r\nV,<V>....... View A File Archive,,5,G\r\nD,<D>.......... Download A File,,5,L\r\nU,<U>............ Upload A File,,5,I\r\nN,<N>................ New Files,,5,N\r\nT,<T>.. Text Search Description,,5,S\r\nF,<F>.............. Find A File,,5,P\r\nM,<M>.......... Message Section,,5,E\r\nQ,<Q>........ Quit To Main Menu,,5,C\r\n@,<@>.......... Sysop Utilities,,50,F\r\nX,<X>........ Xpert Mode Toggle,,5,B\r\nG,<G>........ Goodbye & Log Off,,5,A\r\n?,<?>....... HELP With Commands,,5,?\r\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureReport {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub database_path: PathBuf,
    pub schema_version: u32,
}

pub fn initialize_fixture_board(root: &Path) -> Result<FixtureReport, ApplicationError> {
    if root.exists() {
        return Err(ApplicationError::FixtureExists(root.to_path_buf()));
    }
    if let Some(parent) = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| ApplicationError::CreateFixtureDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir(root).map_err(|source| ApplicationError::CreateFixtureDirectory {
        path: root.to_path_buf(),
        source,
    })?;

    let config = RuntimeConfig::synthetic_fixture();
    let validated = config.validate()?;
    let paths = LogicalPaths::resolve(root, &validated)?;
    paths.create_directories()?;

    let config_path = root.join(FIXTURE_CONFIG_FILE);
    write_new(&config_path, config.to_toml()?.as_bytes())?;
    write_default_resources(&paths, true)?;

    let mut database = RuntimeDatabase::open(paths.database())?;
    database.migrate()?;
    database.ensure_board_identity(&validated.identity)?;
    seed_fixture_messages(&mut database)?;
    let storage = FileStorage::new(&paths)?;
    let areas = seed_fixture_file_areas(&mut database, &storage)?;
    seed_starter_files(&mut database, &storage, &areas)?;
    let schema_version = database.schema_version()?;

    Ok(FixtureReport {
        root: paths.root().to_path_buf(),
        config_path,
        database_path: paths.database().to_path_buf(),
        schema_version,
    })
}

pub(crate) fn seed_fixture_file_areas(
    database: &mut RuntimeDatabase,
    storage: &FileStorage,
) -> Result<Vec<FileArea>, ApplicationError> {
    let security = sf_core::SecurityLevel::new(5).map_err(sf_core::DatabaseError::from)?;
    let mut areas = Vec::new();
    for definition in [
        FileAreaDefinition {
            number: 1,
            name: "General Files".to_owned(),
            description: "General synthetic fixture files".to_owned(),
            storage_key: "general".to_owned(),
            access_mode: FileAccessMode::AtLeast,
            read_security: security,
            upload_security: security,
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: Vec::new(),
        },
        FileAreaDefinition {
            number: 2,
            name: "SPITFIRE Files".to_owned(),
            description: "SPITFIRE NG fixture information".to_owned(),
            storage_key: "spitfire".to_owned(),
            access_mode: FileAccessMode::AtLeast,
            read_security: security,
            upload_security: security,
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: Vec::new(),
        },
    ] {
        let area = database.ensure_file_area(&definition)?;
        storage.ensure_area(&area)?;
        areas.push(area);
    }
    Ok(areas)
}

pub(crate) fn seed_starter_files(
    database: &mut RuntimeDatabase,
    storage: &FileStorage,
    areas: &[FileArea],
) -> Result<(), ApplicationError> {
    let definitions = [
        (
            1,
            "WELCOME.TXT",
            "Welcome to the SPITFIRE NG file library",
            b"Welcome to the SPITFIRE NG file library.\r\nPlease check back for new files and announcements.\r\n".as_slice(),
        ),
        (
            2,
            "SFNGINFO.TXT",
            "About SPITFIRE NG",
            b"SPITFIRE NG Bulletin Board System\r\nVisit the About/Credits menu for project and historical information.\r\n".as_slice(),
        ),
    ];
    for (number, filename, description, contents) in definitions {
        let area = areas.iter().find(|area| area.number == number).ok_or(
            ApplicationError::InvalidSetupValue("missing starter file area"),
        )?;
        if database.file_count(area.id)? == 0 {
            storage.write_seed_file(
                database,
                area,
                filename,
                description,
                contents,
                1_700_000_100 + i64::from(number),
            )?;
        }
    }
    Ok(())
}

pub(crate) fn seed_fixture_messages(
    database: &mut RuntimeDatabase,
) -> Result<(), ApplicationError> {
    let read = sf_core::SecurityLevel::new(5).map_err(sf_core::DatabaseError::from)?;
    let post = sf_core::SecurityLevel::new(5).map_err(sf_core::DatabaseError::from)?;
    database.ensure_conference(&ConferenceDefinition {
        number: 1,
        name: "General".to_owned(),
        description: "General fixture-board discussion".to_owned(),
        access_mode: ConferenceAccessMode::AtLeast,
        read_security: read,
        post_security: post,
        public_only: false,
        caller_deletion_enabled: true,
        maximum_lines: 50,
        privileged_security_levels: Vec::new(),
    })?;
    database.ensure_conference(&ConferenceDefinition {
        number: 2,
        name: "SPITFIRE".to_owned(),
        description: "SPITFIRE NG development discussion".to_owned(),
        access_mode: ConferenceAccessMode::AtLeast,
        read_security: read,
        post_security: post,
        public_only: false,
        caller_deletion_enabled: true,
        maximum_lines: 50,
        privileged_security_levels: Vec::new(),
    })?;
    database.ensure_system_message(
        1,
        b"Welcome to the fixture board",
        b"Welcome to the synthetic SPITFIRE NG fixture board.\r\nThis message is committed test content, not an original Buffalo Creek asset.\r\n",
        1_700_000_000,
    )?;
    database.ensure_system_message(
        2,
        b"SPITFIRE NG development",
        b"This conference is a synthetic place to discuss SPITFIRE NG development.\r\n",
        1_700_000_001,
    )?;
    Ok(())
}

pub(crate) fn write_default_resources(
    paths: &LogicalPaths,
    fixture: bool,
) -> Result<(), ApplicationError> {
    let system = paths.get(LogicalPath::System);
    let board_display = paths.get(LogicalPath::Display);
    sf_core::install_embedded_en_us(system)?;
    let profile = profile_root(paths, MODERN_PROFILE_ID);
    let display = profile.join("resources/display");
    let help_directory = profile.join("resources/help");
    let licenses = profile.join("LICENSES");
    for directory in [&display, &help_directory, &licenses] {
        fs::create_dir_all(directory).map_err(|source| {
            ApplicationError::CreateFixtureDirectory {
                path: directory.to_path_buf(),
                source,
            }
        })?;
    }
    let notice = if fixture {
        b"Synthetic SPITFIRE NG development resources.\nThese are not Buffalo Creek historical assets.\n".as_slice()
    } else {
        b"Generated SPITFIRE NG starter resources.\nThese are not Buffalo Creek historical assets.\n".as_slice()
    };
    write_new(&profile.join(DISPLAY_NOTICE_FILE), notice)?;
    write_new(
        &profile.join("README.md"),
        b"# Modern SPITFIRE NG 1.4.0\n\nThe default project-authored presentation. Version 1.4.0 adds bounded text/archive inspection and private file-request framing while board-owned file authority remains authoritative. The package declares MIT OR Apache-2.0 provenance; no Buffalo Creek resource bytes are included.\n",
    )?;
    write_new(&licenses.join("ASSET-LICENSE.txt"), PROJECT_ASSET_LICENSE)?;
    write_new(&system.join("SFMAIN.MNU"), MAIN_MENU)?;
    write_new(&system.join("SFMSG.MNU"), MESSAGE_MENU)?;
    write_new(&system.join("SFFILE.MNU"), FILE_MENU)?;
    write_new(
        &system.join("SFSYSOP.MNU"),
        crate::resources::BUILTIN_SYSOP_MENU,
    )?;
    write_new(
        &help_directory.join("SPITFIRE.HLP"),
        &synthetic_help_bytes()?,
    )?;

    for (name, body) in [
        (
            "SFPRELOG.BBS",
            b"@CLS@SPITFIRE NG connection established.\r\n".as_slice(),
        ),
        (
            "WELCOME1.BBS",
            b"Welcome to @BOARD@\r\nPlease identify yourself to enter the board.\r\n".as_slice(),
        ),
        (
            "WELCOME2.BBS",
            b"Welcome back, @FNAME@. Security @SLEVEL@; @LOGTIME@ minutes available.\r\n".as_slice(),
        ),
        (
            "NEWUSER.BBS",
            b"Welcome, new caller. Your SPITFIRE account is ready.\r\n".as_slice(),
        ),
        (
            "SFONFAIL.BBS",
            b"That caller name/password was not accepted. Please try again.\r\n".as_slice(),
        ),
        (
            "MAIN10.BBS",
            b"\r\n>>>>>>>> MAIN MENU <<<<<<<<\r\n<M> Messages <F> Files <C> Comment <P> Page <Y> Statistics <R> Profile <U> Terminal\r\n<B> Bulletins <#> Directory <L> Locate <T> System <N> Newsletter <O> Other BBS <A> Add BBS\r\n<V> About <X> Xpert <G> Goodbye <?> Help\r\n".as_slice(),
        ),
        (
            "MSG10.BBS",
            b"\r\n>>>>>>> MESSAGE MENU <<<<<<<\r\n<C> Change Conference  <R> Read  <B> Browse  <E> Enter\r\n<Y> Your Messages      <A> Alter Queue\r\n<S> Caller Search      <T> Text Search\r\n<F> Files <Q> Main      <?> Help\r\n".as_slice(),
        ),
        (
            "FILE10.BBS",
            b"\r\n>>>>>>>> FILE MENU <<<<<<<<<\r\n<C> Change Area  <L> List  <F> Find  <T> Search  <N> New\r\n<R> Read Text    <V> View ZIP <D> Download <U> Upload\r\n<M> Messages     <Q> Main <?> Help\r\n".as_slice(),
        ),
        (
            "SOP50.BBS",
            b"\r\n>>>>>>>> SYSOP MENU <<<<<<<<\r\n<Q> Quit to Main  <X> Xpert  <G> Goodbye\r\n"
                .as_slice(),
        ),
        (
            "GOODBYE.BBS",
            b"\r\nThank you for calling @BOARD@. Goodbye!\r\n".as_slice(),
        ),
        (
            "SFPGOFF.BBS",
            b"The Sysop page is currently unavailable. You may leave a Comment to Sysop.\r\n"
                .as_slice(),
        ),
        (
            "SFUNANS.BBS",
            b"The Sysop did not answer your page. You may leave a Comment to Sysop.\r\n"
                .as_slice(),
        ),
        (
            "SFPAGED.BBS",
            b"Your page has already been sent to the Sysop.\r\n".as_slice(),
        ),
        (
            "USERINIT.BBS",
            b"The Sysop answered. Interactive chat is active; /Q ends chat.\r\n"
                .as_slice(),
        ),
        (
            "CHATDONE.BBS",
            b"Sysop chat ended. Returning to the BBS.\r\n".as_slice(),
        ),
        (
            "SF1STM.BBS",
            b"Entering the SPITFIRE Message Section.\r\n".as_slice(),
        ),
        (
            "SF1STF.BBS",
            b"Entering the SPITFIRE File Section.\r\n".as_slice(),
        ),
        (
            "SFMSG1.BBS",
            b"General message conference selected.\r\n".as_slice(),
        ),
        (
            "SFMSG2.BBS",
            b"SPITFIRE message conference selected.\r\n".as_slice(),
        ),
        (
            "SFIL1.BBS",
            b"General Files area selected.\r\n".as_slice(),
        ),
        (
            "SFIL2.BBS",
            b"SPITFIRE Files area selected.\r\n".as_slice(),
        ),
        (
            "SFDOWN.BBS",
            b"Preparing your SPITFIRE download.\r\n".as_slice(),
        ),
        (
            "SFUP.BBS",
            b"Preparing your SPITFIRE upload.\r\n".as_slice(),
        ),
        (
            "ABOUT.BBS",
            b"\r\nSPITFIRE NG Bulletin Board System\r\nCopyright (C) 2026 Craig Daters and contributors\r\nSPITFIRE NG Project\r\n\r\nSPITFIRE NG is an independent preservation-driven reimplementation\r\nof the original SPITFIRE Bulletin Board System.\r\n\r\nOriginal SPITFIRE Bulletin Board System\r\nCopyright (C) 1987-2010 by Mike Woltz\r\nBuffalo Creek Software\r\n\r\nSPITFIRE NG is not an official Buffalo Creek Software release.\r\n".as_slice(),
        ),
        (
            "PRIVATE.BBS",
            b"This is a private SPITFIRE board. A pre-authorized caller account is required.\r\n".as_slice(),
        ),
        (
            "LOCKOUT.BBS",
            b"This caller account is not available. Please contact the Sysop.\r\n".as_slice(),
        ),
        (
            "SUBWARN.BBS",
            b"Your SPITFIRE subscription will expire soon. Please contact the Sysop to renew.\r\n".as_slice(),
        ),
        (
            "SFSUBCHG.BBS",
            b"Your SPITFIRE subscription has expired and your access level has changed.\r\n".as_slice(),
        ),
        (
            "TOOMANY.BBS",
            b"Your maximum number of calls for this board day has been reached.\r\n".as_slice(),
        ),
        (
            "SFTIMEUP.BBS",
            b"Your SPITFIRE time allowance has expired. Goodbye.\r\n".as_slice(),
        ),
        (
            "SFASLEEP.BBS",
            b"No activity time limit exceeded. Goodbye.\r\n".as_slice(),
        ),
    ] {
        write_new(&display.join(name), body)?;
    }
    for (name, body) in [
        (
            "WELCOME1.CLR",
            "@CLS@\x1B[1;36mWelcome to @BOARD@\x1B[0m\r\n",
        ),
        (
            "MAIN10.CLR",
            "@CLS@\x1B[1;36m>>>>>>>> MAIN MENU <<<<<<<<\x1B[0m\r\n<M> Messages <F> Files <C> Comment <P> Page <Y> Statistics <R> Profile <U> Terminal\r\n<B> Bulletins <#> Directory <L> Locate <T> System <N> Newsletter <O> Other BBS <A> Add BBS\r\n<V> About <X> Xpert <G> Goodbye <?> Help\r\n",
        ),
        (
            "MSG10.CLR",
            "@CLS@\x1B[1;36m>>>>>>> MESSAGE MENU <<<<<<<\x1B[0m\r\n<C> Change Conference  <R> Read  <B> Browse  <E> Enter\r\n<Y> Your Messages      <A> Alter Queue\r\n<S> Caller Search      <T> Text Search\r\n<F> Files <Q> Main      <?> Help\r\n",
        ),
        (
            "FILE10.CLR",
            "@CLS@\x1B[1;36m>>>>>>>> FILE MENU <<<<<<<<<\x1B[0m\r\n<C> Change Area  <L> List  <F> Find  <T> Search  <N> New\r\n<R> Read Text    <V> View ZIP <D> Download <U> Upload\r\n<M> Messages     <Q> Main <?> Help\r\n",
        ),
        (
            "SOP50.CLR",
            "@CLS@\x1B[1;36m>>>>>>>> SYSOP MENU <<<<<<<<\x1B[0m\r\n<Q> Quit to Main  <X> Xpert  <G> Goodbye\r\n",
        ),
        (
            "GOODBYE.CLR",
            "@CLS@\x1B[1;36mThank you for calling @BOARD@. Goodbye!\x1B[0m\r\n",
        ),
        (
            "SUBWARN.CLR",
            "\x1B[1;33mYour SPITFIRE subscription will expire soon. Contact the Sysop to renew.\x1B[0m\r\n",
        ),
        (
            "SFSUBCHG.CLR",
            "\x1B[1;31mYour SPITFIRE subscription expired and your access level changed.\x1B[0m\r\n",
        ),
    ] {
        write_new(&display.join(name), body.as_bytes())?;
    }
    // M043 board-owned publications live in the established DISPLAY root.
    // These are project-authored starter bytes, not original SPITFIRE assets.
    for (name, body) in [
        ("BULLETIN.BBS", b"Available SPITFIRE bulletins:\r\n".as_slice()),
        ("BULLET1.BBS", b"SPITFIRE NG public information is available from the Main Menu.\r\n".as_slice()),
        ("SFNWSLTR.BBS", b"SPITFIRE NG Newsletter\r\nWelcome to the board's project-authored newsletter.\r\n".as_slice()),
        ("THOUGHTS.NG", b"Privacy is part of a caller's identity.\nBoard information should be useful without exposing private data.\n".as_slice()),
    ] {
        write_new(&board_display.join(name), body)?;
    }
    write_modern_profile_descriptor(&profile)?;
    write_minimal_profile(paths, fixture)?;
    write_classic_profile(paths, fixture)?;
    Ok(())
}

fn write_minimal_profile(paths: &LogicalPaths, fixture: bool) -> Result<(), ApplicationError> {
    let profile = profile_root(paths, MINIMAL_PROFILE_ID);
    let display = profile.join("resources/display");
    let help_directory = profile.join("resources/help");
    let licenses = profile.join("LICENSES");
    for directory in [&display, &help_directory, &licenses] {
        fs::create_dir_all(directory).map_err(|source| {
            ApplicationError::CreateFixtureDirectory {
                path: directory.to_path_buf(),
                source,
            }
        })?;
    }
    let notice = if fixture {
        b"Synthetic Minimal Terminal profile.\nProject-authored text; no historical assets.\n"
            .as_slice()
    } else {
        b"Generated Minimal Terminal profile.\nProject-authored text; no historical assets.\n"
            .as_slice()
    };
    write_new(&profile.join(DISPLAY_NOTICE_FILE), notice)?;
    write_new(
        &profile.join("README.md"),
        b"# Minimal Terminal 1.4.0\n\nThe project-authored text-first presentation. Version 1.4.0 adds bounded text/archive inspection and private file-request framing while board-owned file authority remains authoritative. The package declares MIT OR Apache-2.0 provenance; no Buffalo Creek resource bytes are included.\n",
    )?;
    write_new(&licenses.join("ASSET-LICENSE.txt"), PROJECT_ASSET_LICENSE)?;
    write_new(&help_directory.join("SPITFIRE.HLP"), &minimal_help_bytes()?)?;

    for (name, body) in [
        (
            "SFPRELOG.BBS",
            b"SPITFIRE NG\r\nMinimal Terminal profile\r\n".as_slice(),
        ),
        (
            "WELCOME1.BBS",
            b"Welcome to @BOARD@\r\nNode @NODE@\r\nLog in or register to continue.\r\n".as_slice(),
        ),
        (
            "WELCOME2.BBS",
            b"Welcome back, @FNAME@.\r\nSecurity: @SLEVEL@\r\nMinutes available: @LOGTIME@\r\n"
                .as_slice(),
        ),
        (
            "NEWUSER.BBS",
            b"Registration complete.\r\nYour SPITFIRE account is ready.\r\n".as_slice(),
        ),
        (
            "SFONFAIL.BBS",
            b"Caller name or password not accepted.\r\nPlease try again.\r\n".as_slice(),
        ),
        (
            "MAIN10.BBS",
            b"\r\nSPITFIRE NG - MAIN MENU\r\nM Messages | F Files | C Comment | P Page\r\nY Statistics | R Profile | U Terminal\r\nB Bulletins | # Directory | L Locate\r\nT System | N Newsletter | O Other BBS\r\nA Add BBS | V About | X Xpert\r\nG Goodbye | ? Help\r\n"
                .as_slice(),
        ),
        (
            "MAIN50.BBS",
            b"\r\nSPITFIRE NG - MAIN MENU\r\nM Messages | F Files | C Comment | P Page\r\nY Statistics | R Profile | U Terminal\r\nB Bulletins | # Directory | L Locate\r\nT System | N Newsletter | O Other BBS\r\nA Add BBS | V About | @ Sysop | X Xpert\r\nG Goodbye | ? Help\r\n"
                .as_slice(),
        ),
        (
            "MSG10.BBS",
            b"\r\nSPITFIRE NG - MESSAGE MENU\r\nC - Change conference\r\nR - Read messages\r\nB - Browse messages\r\nE - Enter a message\r\nY - Your messages\r\nA - Alter conference queue\r\nS - Specific caller messages\r\nT - Text search\r\nF - Files\r\nQ - Main menu\r\nX - Toggle expert mode\r\nG - Goodbye\r\n? - Help\r\n"
                .as_slice(),
        ),
        (
            "MSG50.BBS",
            b"\r\nSPITFIRE NG - MESSAGE MENU\r\nC - Change conference\r\nR - Read messages\r\nB - Browse messages\r\nE - Enter a message\r\nY - Your messages\r\nA - Alter conference queue\r\nS - Specific caller messages\r\nT - Text search\r\nF - Files\r\nQ - Main menu\r\n@ - Sysop utilities\r\nX - Toggle expert mode\r\nG - Goodbye\r\n? - Help\r\n"
                .as_slice(),
        ),
        (
            "FILE10.BBS",
            b"\r\nSPITFIRE NG - FILE MENU\r\nC - Change file area\r\nL - List files\r\nD - Download a file\r\nU - Upload a file\r\nN - New files\r\nT - Search descriptions\r\nF - Find a file\r\nM - Messages\r\nQ - Main menu\r\nX - Toggle expert mode\r\nG - Goodbye\r\n? - Help\r\n"
                .as_slice(),
        ),
        (
            "FILE50.BBS",
            b"\r\nSPITFIRE NG - FILE MENU\r\nC - Change file area\r\nL - List files\r\nD - Download a file\r\nU - Upload a file\r\nN - New files\r\nT - Search descriptions\r\nF - Find a file\r\nM - Messages\r\nQ - Main menu\r\n@ - Sysop utilities\r\nX - Toggle expert mode\r\nG - Goodbye\r\n? - Help\r\n"
                .as_slice(),
        ),
        (
            "SOP50.BBS",
            b"\r\nSPITFIRE NG - SYSOP MENU\r\nQ - Main menu\r\nX - Toggle expert mode\r\nG - Goodbye\r\n"
                .as_slice(),
        ),
        (
            "GOODBYE.BBS",
            b"Thank you for calling @BOARD@.\r\nGoodbye.\r\n".as_slice(),
        ),
        (
            "SFPGOFF.BBS",
            b"The Sysop page is unavailable.\r\nYou may leave a Comment to Sysop.\r\n"
                .as_slice(),
        ),
        (
            "SFUNANS.BBS",
            b"The Sysop did not answer.\r\nYou may leave a Comment to Sysop.\r\n"
                .as_slice(),
        ),
        (
            "SFPAGED.BBS",
            b"Your page has already been sent.\r\n".as_slice(),
        ),
        (
            "USERINIT.BBS",
            b"The Sysop answered.\r\nChat is active. Enter /Q to finish.\r\n".as_slice(),
        ),
        (
            "CHATDONE.BBS",
            b"Sysop chat ended.\r\nReturning to the board.\r\n".as_slice(),
        ),
        (
            "SF1STM.BBS",
            b"Entering the Message section.\r\n".as_slice(),
        ),
        (
            "SF1STF.BBS",
            b"Entering the File section.\r\n".as_slice(),
        ),
        (
            "SFMSG1.BBS",
            b"Message conference: General\r\n".as_slice(),
        ),
        (
            "SFMSG2.BBS",
            b"Message conference: SPITFIRE\r\n".as_slice(),
        ),
        ("SFIL1.BBS", b"File area: General Files\r\n".as_slice()),
        ("SFIL2.BBS", b"File area: SPITFIRE Files\r\n".as_slice()),
        (
            "SFDOWN.BBS",
            b"Preparing the requested download.\r\n".as_slice(),
        ),
        (
            "SFUP.BBS",
            b"Preparing the requested upload.\r\n".as_slice(),
        ),
        (
            "ABOUT.BBS",
            b"\r\nSPITFIRE NG Bulletin Board System\r\nMinimal Terminal profile\r\n\r\nSPITFIRE NG is an independent,\r\npreservation-driven reimplementation.\r\n\r\nOriginal SPITFIRE Bulletin Board System\r\nCopyright (C) 1987-2010 Mike Woltz\r\nBuffalo Creek Software\r\n\r\nSPITFIRE NG Project\r\nCopyright (C) 2026 Craig Daters\r\nand contributors\r\n\r\nNot an official Buffalo Creek release.\r\n"
                .as_slice(),
        ),
        (
            "PRIVATE.BBS",
            b"This board is private.\r\nA pre-authorized account is required.\r\n".as_slice(),
        ),
        (
            "LOCKOUT.BBS",
            b"This caller account is unavailable.\r\nContact the Sysop for help.\r\n".as_slice(),
        ),
        (
            "SUBWARN.BBS",
            b"Your subscription will expire soon.\r\nContact the Sysop to renew.\r\n".as_slice(),
        ),
        (
            "SFSUBCHG.BBS",
            b"Your subscription expired.\r\nYour access level has changed.\r\n".as_slice(),
        ),
        (
            "TOOMANY.BBS",
            b"The daily call limit has been reached.\r\n".as_slice(),
        ),
        (
            "SFTIMEUP.BBS",
            b"Your time allowance has expired.\r\nGoodbye.\r\n".as_slice(),
        ),
        (
            "SFASLEEP.BBS",
            b"The inactivity limit has been reached.\r\nGoodbye.\r\n".as_slice(),
        ),
    ] {
        write_new(&display.join(name), body)?;
    }
    write_minimal_profile_descriptor(&profile)
}

fn write_classic_profile(paths: &LogicalPaths, fixture: bool) -> Result<(), ApplicationError> {
    let profile = profile_root(paths, CLASSIC_PROFILE_ID);
    let display = profile.join("resources/display");
    let help_directory = profile.join("resources/help");
    let licenses = profile.join("LICENSES");
    for directory in [&display, &help_directory, &licenses] {
        fs::create_dir_all(directory).map_err(|source| {
            ApplicationError::CreateFixtureDirectory {
                path: directory.to_path_buf(),
                source,
            }
        })?;
    }
    let notice = if fixture {
        b"Synthetic Classic SPITFIRE-inspired profile.\nIndependently authored project assets; no historical bytes.\n".as_slice()
    } else {
        b"Generated Classic SPITFIRE-inspired profile.\nIndependently authored project assets; no historical bytes.\n".as_slice()
    };
    write_new(&profile.join(DISPLAY_NOTICE_FILE), notice)?;
    write_new(
        &profile.join("README.md"),
        b"# Classic SPITFIRE-Inspired 1.5.0\n\nAn independently authored presentation for SPITFIRE NG. It is not an original SPITFIRE 3.7 package or an official Buffalo Creek Software release. Version 1.5.0 adds bounded text/archive inspection and private file-request framing; modern SPITFIRE NG authorization, privacy, storage, and transport behavior remain authoritative.\n",
    )?;
    write_new(
        &licenses.join("ASSET-LICENSE.txt"),
        b"SPDX-License-Identifier: MIT OR Apache-2.0\n\nCopyright (C) 2026 Craig Daters and SPITFIRE NG contributors.\nThese independently authored resources are available under the SPITFIRE NG repository's LICENSE-MIT or LICENSE-APACHE terms, at your option.\nNo original Buffalo Creek DISPLAY, HLP, MNU, or RIP bytes are included or relicensed. Historical evidence and third-party research retain their original copyrights and licenses.\n",
    )?;
    write_new(&help_directory.join("SPITFIRE.HLP"), &classic_help_bytes()?)?;

    write_new(&display.join("SFPRELOG.BBS"), &classic_prelogin())?;
    write_classic_pair(&display, "WELCOME1", classic_welcome)?;
    write_classic_pair(&display, "GOODBYE", classic_goodbye)?;

    for stem in [
        "MAIN10", "MAIN50", "MSG10", "MSG50", "FILE10", "FILE50", "SOP50",
    ] {
        let bbs = classic_menu(stem, false);
        let clr = classic_menu(stem, true);
        write_new(&display.join(format!("{stem}.BBS")), &bbs)?;
        write_new(&display.join(format!("{stem}.CLR")), &clr)?;
    }

    for (stem, title, lines, clear) in classic_display_plan() {
        let uninterrupted = *clear || *stem == "WELCOME2";
        let bbs = classic_panel(stem, title, lines, *clear, false, uninterrupted);
        let clr = classic_panel(stem, title, lines, *clear, true, uninterrupted);
        write_new(&display.join(format!("{stem}.BBS")), &bbs)?;
        write_new(&display.join(format!("{stem}.CLR")), &clr)?;
    }
    write_classic_profile_descriptor(&profile)
}

fn classic_display_plan() -> &'static [(&'static str, &'static str, &'static [&'static str], bool)]
{
    &[
        (
            "WELCOME2",
            "WELCOME BACK TO SPITFIRE NG",
            &[
                "Welcome back, @FNAME@.",
                "Security level @SLEVEL@ - @LOGTIME@ minute(s) available this call",
                "Reviewing your current message and file activity...",
            ],
            false,
        ),
        (
            "NEWUSER",
            "NEW CALLER WELCOME",
            &[
                "Your caller account is ready. Welcome aboard!",
                "Use <?> for HELP and <U> for terminal preferences.",
                "The Main Menu is your starting point for every section.",
            ],
            false,
        ),
        (
            "SFONFAIL",
            "LOGON NOT ACCEPTED",
            &[
                "That caller name or password was not accepted.",
                "Please check your entry and try again.",
            ],
            false,
        ),
        (
            "SFPGOFF",
            "SYSOP PAGE",
            &[
                "The Sysop page is currently unavailable.",
                "You may leave a Comment to Sysop from Main.",
            ],
            false,
        ),
        (
            "SFUNANS",
            "SYSOP PAGE",
            &[
                "The Sysop did not answer your page.",
                "You may leave a Comment to Sysop from Main.",
            ],
            false,
        ),
        (
            "SFPAGED",
            "SYSOP PAGE",
            &["Your page has already been sent to the Sysop."],
            false,
        ),
        (
            "USERINIT",
            "SYSOP CHAT",
            &[
                "The Sysop answered. Live text chat is active.",
                "Enter /Q on a line by itself to finish.",
            ],
            false,
        ),
        (
            "CHATDONE",
            "SYSOP CHAT",
            &["Sysop chat ended. Returning to the board."],
            false,
        ),
        (
            "SF1STM",
            "MESSAGE SECTION",
            &["Entering the SPITFIRE Message Section."],
            false,
        ),
        (
            "SF1STF",
            "FILE SECTION",
            &["Entering the SPITFIRE File Section."],
            false,
        ),
        (
            "SFMSG1",
            "MESSAGE CONFERENCE",
            &["General conference selected."],
            false,
        ),
        (
            "SFMSG2",
            "MESSAGE CONFERENCE",
            &["SPITFIRE conference selected."],
            false,
        ),
        (
            "SFIL1",
            "FILE AREA",
            &["General Files area selected."],
            false,
        ),
        (
            "SFIL2",
            "FILE AREA",
            &["SPITFIRE Files area selected."],
            false,
        ),
        (
            "SFDOWN",
            "FILE TRANSFER",
            &[
                "Your download is being prepared.",
                "Select your configured protocol when prompted.",
            ],
            false,
        ),
        (
            "SFUP",
            "FILE TRANSFER",
            &[
                "Your upload is ready to begin.",
                "The file will appear after verification is complete.",
            ],
            false,
        ),
        (
            "ABOUT",
            "ABOUT SPITFIRE NG",
            &[
                "SPITFIRE NG Bulletin Board System",
                "Copyright (C) 2026 Craig Daters and contributors",
                "An independent preservation-driven reimplementation.",
                "",
                "Original SPITFIRE Bulletin Board System",
                "Copyright (C) 1987-2010 Mike Woltz",
                "Buffalo Creek Software",
                "",
                "Classic SPITFIRE-Inspired presentation 1.5.0",
                "Not an official Buffalo Creek Software release or endorsement.",
            ],
            false,
        ),
        (
            "PRIVATE",
            "PRIVATE SYSTEM",
            &[
                "This board requires a pre-authorized caller account.",
                "Contact the configured Sysop for access.",
            ],
            false,
        ),
        (
            "LOCKOUT",
            "CALLER UNAVAILABLE",
            &[
                "This caller account is unavailable.",
                "Contact the Sysop for assistance.",
            ],
            false,
        ),
        (
            "SUBWARN",
            "SUBSCRIPTION WARNING",
            &[
                "Your SPITFIRE subscription will expire soon.",
                "Contact the Sysop to renew your access.",
            ],
            false,
        ),
        (
            "SFSUBCHG",
            "SUBSCRIPTION CHANGE",
            &[
                "Your SPITFIRE subscription has expired.",
                "Your access level has changed.",
            ],
            false,
        ),
        (
            "TOOMANY",
            "CALL LIMIT",
            &["Your maximum calls for this board day have been reached."],
            false,
        ),
        (
            "SFTIMEUP",
            "TIME LIMIT",
            &["Your SPITFIRE NG time allowance has expired.", "Goodbye."],
            false,
        ),
        (
            "SFASLEEP",
            "NO ACTIVITY",
            &["The no-activity time limit has been exceeded.", "Goodbye."],
            false,
        ),
    ]
}

fn write_classic_pair(
    display: &Path,
    stem: &str,
    render: fn(bool) -> Vec<u8>,
) -> Result<(), ApplicationError> {
    write_new(&display.join(format!("{stem}.BBS")), &render(false))?;
    write_new(&display.join(format!("{stem}.CLR")), &render(true))
}

fn classic_prelogin() -> Vec<u8> {
    let mut output = b"@PROMPTOFF@@CLS@".to_vec();
    classic_rule(&mut output, 0xCD, 78);
    output.extend_from_slice(b"SPITFIRE NG - NODE @NODE@\r\n");
    output.extend_from_slice(b"@BOARD@\r\n");
    output.extend_from_slice(b"Your Sysop is @SYSOP@\r\n");
    classic_rule(&mut output, 0xCD, 78);
    output
}

fn classic_welcome(ansi: bool) -> Vec<u8> {
    let mut output = b"@PROMPTOFF@@CLS@".to_vec();
    if ansi {
        output.extend_from_slice(b"\x1B[1;33m");
    }
    classic_centered_rule(&mut output, "WELCOME TO", 0xCD, 78);
    if ansi {
        output.extend_from_slice(b"\x1B[1;34m");
    }
    classic_centered_line(&mut output, "S P I T F I R E   N G", 78);
    if ansi {
        output.extend_from_slice(b"\x1B[1;36m");
    }
    classic_centered_line(&mut output, "BULLETIN BOARD SYSTEM", 78);
    if ansi {
        output.extend_from_slice(b"\x1B[0;35m");
    }
    classic_rule(&mut output, 0xC4, 66);
    if ansi {
        output.extend_from_slice(b"\x1B[1;37m");
    }
    output.extend_from_slice(b"@BOARD@\r\n");
    output.extend_from_slice(b"Your Sysop is @SYSOP@ - Node @NODE@\r\n");
    output.extend_from_slice(b"Please enter your caller information to continue.\r\n");
    if ansi {
        output.extend_from_slice(b"\x1B[0m");
    }
    output
}

fn classic_goodbye(ansi: bool) -> Vec<u8> {
    let mut output = b"@PROMPTOFF@@CLS@".to_vec();
    if ansi {
        output.extend_from_slice(b"\x1B[1;35m");
    }
    classic_centered_rule(&mut output, "THANK YOU FOR CALLING", 0xCD, 78);
    if ansi {
        output.extend_from_slice(b"\x1B[1;34m");
    }
    classic_centered_line(&mut output, "S P I T F I R E   N G", 78);
    if ansi {
        output.extend_from_slice(b"\x1B[1;36m");
    }
    classic_centered_line(&mut output, "Your connection with", 78);
    if ansi {
        output.extend_from_slice(b"\x1B[1;37m");
    }
    output.extend_from_slice(b"@BOARD@\r\n");
    if ansi {
        output.extend_from_slice(b"\x1B[1;33m");
    }
    classic_centered_line(&mut output, "is now complete.", 78);
    classic_centered_line(&mut output, "Please call again soon!", 78);
    if ansi {
        output.extend_from_slice(b"\x1B[0;35m");
    }
    classic_rule(&mut output, 0xCD, 78);
    if ansi {
        output.extend_from_slice(b"\x1B[0m");
    }
    output
}

fn classic_menu(stem: &str, ansi: bool) -> Vec<u8> {
    let (title, lines, style) = match stem {
        "MAIN10" => (
            "SPITFIRE MAIN MENU",
            CLASSIC_MAIN10.as_slice(),
            ClassicMenuStyle::MainCaller,
        ),
        "MAIN50" => (
            "SPITFIRE NG SYSOP MAIN",
            CLASSIC_MAIN50.as_slice(),
            ClassicMenuStyle::MainSysop,
        ),
        "MSG10" => (
            "SPITFIRE MESSAGE MENU",
            CLASSIC_MSG10.as_slice(),
            ClassicMenuStyle::Message,
        ),
        "MSG50" => (
            "SPITFIRE MESSAGE MENU",
            CLASSIC_MSG50.as_slice(),
            ClassicMenuStyle::Message,
        ),
        "FILE10" => (
            "SPITFIRE FILE MENU",
            CLASSIC_FILE10.as_slice(),
            ClassicMenuStyle::File,
        ),
        "FILE50" => (
            "SPITFIRE FILE MENU",
            CLASSIC_FILE50.as_slice(),
            ClassicMenuStyle::File,
        ),
        "SOP50" => (
            "SPITFIRE SYSOP MENU",
            CLASSIC_SOP50.as_slice(),
            ClassicMenuStyle::Sysop,
        ),
        _ => unreachable!("unknown Classic menu stem"),
    };
    if ansi {
        classic_ansi_menu(title, lines, style)
    } else {
        classic_bbs_menu(title, lines, style)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ClassicMenuStyle {
    MainCaller,
    MainSysop,
    Message,
    File,
    Sysop,
}

const CLASSIC_MAIN10: [&str; 7] = [
    "<M>....... Message Section       <F>.......... File Section",
    "<C>...... Comment to Sysop       <P>......... Page the Sysop",
    "<Y>....... Your Statistics       <R>......... Caller Profile",
    "<U>.. Terminal Preferences       <V>..... About SPITFIRE NG",
    "<B> Bulletins <#> Directory <L> Locate <T> System",
    "<N> Newsletter <O> Other BBS <A> Add BBS",
    "<X> Xpert <G> Goodbye <?> HELP",
];
const CLASSIC_MAIN50: [&str; 7] = [
    "<M>....... Message Section       <F>.......... File Section",
    "<C>...... Comment to Sysop       <P>......... Page the Sysop",
    "<Y>....... Your Statistics       <R>......... Caller Profile",
    "<U>.. Terminal Preferences       <V>..... About SPITFIRE NG",
    "<B> Bulletins <#> Directory <L> Locate <T> System",
    "<N> Newsletter <O> Other BBS <A> Add BBS",
    "<@> Sysop <X> Xpert <G> Goodbye <?> HELP",
];
const CLASSIC_MSG10: [&str; 7] = [
    "<C>... Change Conference         <R>......... Read Messages",
    "<B>...... Browse Messages        <E>.... Enter New Message",
    "<Y>........ Your Messages        <A>.. Alter Conf. Queue",
    "<S> Specific Caller Msgs         <T>.......... Text Search",
    "<F>......... File Section        <Q>.... Quit to Main Menu",
    "<X>.......... Xpert Mode         <G>..... Goodbye & Log Off",
    "<?>.. HELP With Commands",
];
const CLASSIC_MSG50: [&str; 7] = [
    "<C>... Change Conference         <R>......... Read Messages",
    "<B>...... Browse Messages        <E>.... Enter New Message",
    "<Y>........ Your Messages        <A>.. Alter Conf. Queue",
    "<S> Specific Caller Msgs         <T>.......... Text Search",
    "<F>......... File Section        <Q>.... Quit to Main Menu",
    "<@>...... Sysop Utilities        <X>.......... Xpert Mode",
    "<G>..... Goodbye & Log Off       <?>.. HELP With Commands",
];
const CLASSIC_FILE10: [&str; 7] = [
    "<C>..... Change File Area        <L>............ List Files",
    "<R>..... Read A Text File        <V>... View File Archive",
    "<D>..... Download a File         <U>........ Upload a File",
    "<N>............ New Files        <T>.. Search Descriptions",
    "<F>.......... Find a File        <M>....... Message Section",
    "<Q>.... Quit to Main Menu        <X>.......... Xpert Mode",
    "<G>..... Goodbye & Log Off       <?>.. HELP With Commands",
];
const CLASSIC_FILE50: [&str; 8] = [
    "<C>..... Change File Area        <L>............ List Files",
    "<R>..... Read A Text File        <V>... View File Archive",
    "<D>..... Download a File         <U>........ Upload a File",
    "<N>............ New Files        <T>.. Search Descriptions",
    "<F>.......... Find a File        <M>....... Message Section",
    "<Q>.... Quit to Main Menu        <@>...... Sysop Utilities",
    "<X>.......... Xpert Mode         <G>..... Goodbye & Log Off",
    "<?>.. HELP With Commands",
];
const CLASSIC_SOP50: [&str; 2] = [
    "<Q>.... Quit to Main Menu        <X>.......... Xpert Mode",
    "<G>..... Goodbye & Log Off",
];

fn classic_bbs_menu(title: &str, lines: &[&str], style: ClassicMenuStyle) -> Vec<u8> {
    let mut output = b"@PROMPTOFF@@CLS@".to_vec();
    if matches!(
        style,
        ClassicMenuStyle::MainCaller | ClassicMenuStyle::MainSysop
    ) {
        output.push(0xC9);
        output.extend(std::iter::repeat_n(0xCD, 76));
        output.extend_from_slice(&[0xBB, b'\r', b'\n']);
        classic_box_row(&mut output, title, 76, true);
        output.push(0xCC);
        output.extend(std::iter::repeat_n(0xCD, 76));
        output.extend_from_slice(&[0xB9, b'\r', b'\n']);
        for line in lines {
            classic_box_row(&mut output, line, 76, false);
        }
        output.push(0xC8);
        output.extend(std::iter::repeat_n(0xCD, 76));
        output.extend_from_slice(&[0xBC, b'\r', b'\n']);
    } else {
        classic_chevron_title(&mut output, title);
        for line in lines {
            classic_plain_line(&mut output, line);
        }
        classic_rule(&mut output, 0xC4, 72);
    }
    output
}

fn classic_ansi_menu(title: &str, lines: &[&str], style: ClassicMenuStyle) -> Vec<u8> {
    let mut output = b"@PROMPTOFF@@CLS@".to_vec();
    let (outer, inner) = match style {
        ClassicMenuStyle::MainCaller => (b"\x1B[1;37;40m".as_slice(), b"\x1B[1;37;45m".as_slice()),
        ClassicMenuStyle::MainSysop => (b"\x1B[1;37;40m".as_slice(), b"\x1B[1;37;44m".as_slice()),
        ClassicMenuStyle::Message => (b"\x1B[1;37;44m".as_slice(), b"\x1B[1;37;41m".as_slice()),
        ClassicMenuStyle::File => (b"\x1B[1;33;41m".as_slice(), b"\x1B[1;37;44m".as_slice()),
        ClassicMenuStyle::Sysop => (b"\x1B[1;33;41m".as_slice(), b"\x1B[1;30;42m".as_slice()),
    };
    classic_ansi_band(&mut output, "", 76, 1, outer);
    classic_ansi_band(&mut output, title, 76, 1, outer);
    classic_ansi_band(&mut output, "", 72, 3, inner);
    for line in lines {
        classic_ansi_band(&mut output, line, 72, 3, inner);
    }
    classic_ansi_band(&mut output, "", 72, 3, inner);
    classic_ansi_band(&mut output, "", 72, 5, b"\x1B[0;30;40m");
    output.extend_from_slice(b"\x1B[0m");
    output
}

fn classic_ansi_band(output: &mut Vec<u8>, text: &str, width: usize, indent: usize, color: &[u8]) {
    assert!(
        text.len() <= width,
        "Classic ANSI menu line exceeds its band"
    );
    output.extend_from_slice(b"\x1B[0m");
    output.extend(std::iter::repeat_n(b' ', indent));
    output.extend_from_slice(color);
    output.extend_from_slice(text.as_bytes());
    output.extend(std::iter::repeat_n(b' ', width - text.len()));
    output.extend_from_slice(b"\x1B[0m\r\n");
}

fn classic_panel(
    stem: &str,
    title: &str,
    lines: &[&str],
    clear: bool,
    ansi: bool,
    uninterrupted: bool,
) -> Vec<u8> {
    const INNER: usize = 76;
    let mut output = Vec::new();
    if uninterrupted {
        output.extend_from_slice(b"@PROMPTOFF@");
    }
    if clear {
        output.extend_from_slice(b"@CLS@");
    }
    if ansi {
        output.extend_from_slice(classic_panel_color(stem));
    }
    output.push(0xC9);
    output.extend(std::iter::repeat_n(0xCD, INNER));
    output.extend_from_slice(&[0xBB, b'\r', b'\n']);
    classic_box_row(&mut output, title, INNER, true);
    output.push(0xCC);
    output.extend(std::iter::repeat_n(0xCD, INNER));
    output.extend_from_slice(&[0xB9, b'\r', b'\n']);
    for line in lines {
        classic_box_row(&mut output, line, INNER, false);
    }
    output.push(0xC8);
    output.extend(std::iter::repeat_n(0xCD, INNER));
    output.extend_from_slice(&[0xBC, b'\r', b'\n']);
    if ansi {
        output.extend_from_slice(b"\x1B[0m");
    }
    output
}

fn classic_panel_color(stem: &str) -> &'static [u8] {
    match stem {
        "SFONFAIL" | "LOCKOUT" | "PRIVATE" | "TOOMANY" | "SFTIMEUP" | "SFASLEEP" | "SFSUBCHG" => {
            b"\x1B[1;31m"
        }
        "SUBWARN" => b"\x1B[1;33m",
        "SFDOWN" | "SFUP" | "SF1STM" | "SF1STF" | "SFMSG1" | "SFMSG2" | "SFIL1" | "SFIL2" => {
            b"\x1B[1;36m"
        }
        "WELCOME2" | "NEWUSER" | "ABOUT" => b"\x1B[1;33m",
        _ => b"\x1B[0;37m",
    }
}

fn classic_box_row(output: &mut Vec<u8>, text: &str, inner: usize, centered: bool) {
    assert!(
        text.len() <= inner - 2,
        "Classic display line exceeds 74 columns"
    );
    output.push(0xBA);
    let remaining = inner - text.len();
    let left = if centered { remaining / 2 } else { 1 };
    output.extend(std::iter::repeat_n(b' ', left));
    output.extend_from_slice(text.as_bytes());
    if text.contains('@') {
        output.extend_from_slice(b"\r\n");
        return;
    }
    output.extend(std::iter::repeat_n(b' ', remaining - left));
    output.extend_from_slice(&[0xBA, b'\r', b'\n']);
}

fn classic_chevron_title(output: &mut Vec<u8>, title: &str) {
    let line = format!(">>>>>>> {title} <<<<<<<");
    classic_plain_line(output, &line);
}

fn classic_plain_line(output: &mut Vec<u8>, text: &str) {
    assert!(text.len() <= 80, "Classic line exceeds 80 columns");
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(b"\r\n");
}

fn classic_rule(output: &mut Vec<u8>, byte: u8, width: usize) {
    output.extend(std::iter::repeat_n(byte, width));
    output.extend_from_slice(b"\r\n");
}

fn classic_centered_rule(output: &mut Vec<u8>, title: &str, byte: u8, width: usize) {
    assert!(title.len() + 2 <= width);
    let remaining = width - title.len() - 2;
    output.extend(std::iter::repeat_n(byte, remaining / 2));
    output.push(b' ');
    output.extend_from_slice(title.as_bytes());
    output.push(b' ');
    output.extend(std::iter::repeat_n(byte, remaining - remaining / 2));
    output.extend_from_slice(b"\r\n");
}

fn classic_centered_line(output: &mut Vec<u8>, text: &str, width: usize) {
    assert!(text.len() <= width);
    output.extend(std::iter::repeat_n(b' ', (width - text.len()) / 2));
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(b"\r\n");
}

fn write_modern_profile_descriptor(profile: &Path) -> Result<(), ApplicationError> {
    let display = profile.join("resources/display");
    let mut files = fs::read_dir(&display)
        .map_err(|source| ApplicationError::ReadResource {
            path: display.clone(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    files.push(profile.join("resources/help/SPITFIRE.HLP"));
    let mut resources = Vec::new();
    for path in files {
        let bytes = fs::read(&path).map_err(|source| ApplicationError::ReadResource {
            path: path.clone(),
            source,
        })?;
        let name = path.file_name().and_then(|value| value.to_str()).ok_or(
            ApplicationError::InvalidSetupValue("Modern profile resource has a non-UTF-8 name"),
        )?;
        let relative = path
            .strip_prefix(profile)
            .map_err(|_| ApplicationError::InvalidSetupValue("Modern resource escaped profile"))?
            .to_path_buf();
        let (key, kind, format) = if name.eq_ignore_ascii_case("SPITFIRE.HLP") {
            (
                "SPITFIRE.HLP".to_owned(),
                ProfileResourceKind::Help,
                ProfileFormat::SpitfireHelp,
            )
        } else {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or(ApplicationError::InvalidSetupValue(
                    "Modern display has a non-UTF-8 stem",
                ))?
                .to_ascii_uppercase();
            let format = match path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_uppercase)
                .as_deref()
            {
                Some("BBS") => ProfileFormat::Bbs,
                Some("CLR") => ProfileFormat::Clr,
                _ => {
                    return Err(ApplicationError::InvalidSetupValue(
                        "Modern display has an unsupported extension",
                    ))
                }
            };
            let kind = if stem.starts_with("MAIN")
                || stem.starts_with("MSG")
                || stem.starts_with("FILE")
                || stem.starts_with("SOP")
            {
                ProfileResourceKind::MenuArtwork
            } else {
                ProfileResourceKind::Display
            };
            (stem, kind, format)
        };
        resources.push(ProfileResourceRecord {
            key,
            kind,
            format,
            path: relative,
            bytes: bytes.len() as u64,
            sha256: hex_digest(&bytes),
            provenance: "spitfire-ng".to_owned(),
        });
    }
    let descriptor = ProfileDescriptor {
        format_version: PROFILE_FORMAT_VERSION,
        id: MODERN_PROFILE_ID.to_owned(),
        version: crate::MODERN_PROFILE_VERSION.to_owned(),
        display_name: "Modern SPITFIRE NG".to_owned(),
        description: "The unchanged default SPITFIRE NG Development Preview presentation."
            .to_owned(),
        resource_api_version: RESOURCE_API_VERSION,
        engine: EngineCompatibility {
            minimum: "0.1.0".to_owned(),
            maximum_exclusive: "0.2.0".to_owned(),
        },
        compatibility_target: "SPITFIRE NG Development Preview".to_owned(),
        supported_formats: vec![
            ProfileFormat::Bbs,
            ProfileFormat::Clr,
            ProfileFormat::SpitfireHelp,
        ],
        fallback_policy: FallbackPolicy::BaseThenBuiltIn,
        provenance: vec![ProvenanceRecord {
            id: "spitfire-ng".to_owned(),
            kind: ProvenanceKind::ProjectAuthored,
            creator: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            rightsholder: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            source: "SPITFIRE NG source-tree generated starter resources".to_owned(),
            source_hash: None,
            license: "MIT OR Apache-2.0".to_owned(),
            redistribution: Redistribution::Allowed,
            modifications: None,
            evidence: Some("crates/sf-bbs/src/fixture.rs".to_owned()),
        }],
        resources,
    };
    let encoded = toml::to_string_pretty(&descriptor)
        .map_err(|_| ApplicationError::InvalidSetupValue("could not serialize Modern profile"))?;
    write_new(&profile.join(PROFILE_DESCRIPTOR), encoded.as_bytes())
}

fn write_minimal_profile_descriptor(profile: &Path) -> Result<(), ApplicationError> {
    let display = profile.join("resources/display");
    let mut files = fs::read_dir(&display)
        .map_err(|source| ApplicationError::ReadResource {
            path: display.clone(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    files.push(profile.join("resources/help/SPITFIRE.HLP"));
    let mut resources = Vec::new();
    for path in files {
        let bytes = fs::read(&path).map_err(|source| ApplicationError::ReadResource {
            path: path.clone(),
            source,
        })?;
        let name = path.file_name().and_then(|value| value.to_str()).ok_or(
            ApplicationError::InvalidSetupValue("Minimal profile resource has a non-UTF-8 name"),
        )?;
        let relative = path
            .strip_prefix(profile)
            .map_err(|_| ApplicationError::InvalidSetupValue("Minimal resource escaped profile"))?
            .to_path_buf();
        let (key, kind, format) = if name.eq_ignore_ascii_case("SPITFIRE.HLP") {
            (
                "SPITFIRE.HLP".to_owned(),
                ProfileResourceKind::Help,
                ProfileFormat::SpitfireHelp,
            )
        } else {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or(ApplicationError::InvalidSetupValue(
                    "Minimal display has a non-UTF-8 stem",
                ))?
                .to_ascii_uppercase();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_uppercase);
            if extension.as_deref() != Some("BBS") {
                return Err(ApplicationError::InvalidSetupValue(
                    "Minimal display has an unsupported extension",
                ));
            }
            let kind = if stem.starts_with("MAIN")
                || stem.starts_with("MSG")
                || stem.starts_with("FILE")
                || stem.starts_with("SOP")
            {
                ProfileResourceKind::MenuArtwork
            } else {
                ProfileResourceKind::Display
            };
            (stem, kind, ProfileFormat::Bbs)
        };
        resources.push(ProfileResourceRecord {
            key,
            kind,
            format,
            path: relative,
            bytes: bytes.len() as u64,
            sha256: hex_digest(&bytes),
            provenance: "spitfire-ng-minimal".to_owned(),
        });
    }
    let descriptor = ProfileDescriptor {
        format_version: PROFILE_FORMAT_VERSION,
        id: MINIMAL_PROFILE_ID.to_owned(),
        version: crate::MINIMAL_PROFILE_VERSION.to_owned(),
        display_name: "Minimal Terminal".to_owned(),
        description: "A text-first presentation for low-capability and accessible terminals."
            .to_owned(),
        resource_api_version: RESOURCE_API_VERSION,
        engine: EngineCompatibility {
            minimum: "0.1.0".to_owned(),
            maximum_exclusive: "0.2.0".to_owned(),
        },
        compatibility_target: "SPITFIRE NG Minimal Terminal".to_owned(),
        supported_formats: vec![ProfileFormat::Bbs, ProfileFormat::SpitfireHelp],
        fallback_policy: FallbackPolicy::BaseThenBuiltIn,
        provenance: vec![ProvenanceRecord {
            id: "spitfire-ng-minimal".to_owned(),
            kind: ProvenanceKind::ProjectAuthored,
            creator: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            rightsholder: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            source: "SPITFIRE NG source-tree generated Minimal Terminal resources".to_owned(),
            source_hash: None,
            license: "MIT OR Apache-2.0".to_owned(),
            redistribution: Redistribution::Allowed,
            modifications: None,
            evidence: Some("crates/sf-bbs/src/fixture.rs".to_owned()),
        }],
        resources,
    };
    let encoded = toml::to_string_pretty(&descriptor)
        .map_err(|_| ApplicationError::InvalidSetupValue("could not serialize Minimal profile"))?;
    write_new(&profile.join(PROFILE_DESCRIPTOR), encoded.as_bytes())
}

fn write_classic_profile_descriptor(profile: &Path) -> Result<(), ApplicationError> {
    let display = profile.join("resources/display");
    let mut files = fs::read_dir(&display)
        .map_err(|source| ApplicationError::ReadResource {
            path: display.clone(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    files.push(profile.join("resources/help/SPITFIRE.HLP"));
    let mut resources = Vec::new();
    let mut provenance = Vec::new();
    for (index, path) in files.into_iter().enumerate() {
        let bytes = fs::read(&path).map_err(|source| ApplicationError::ReadResource {
            path: path.clone(),
            source,
        })?;
        let name = path.file_name().and_then(|value| value.to_str()).ok_or(
            ApplicationError::InvalidSetupValue("Classic profile resource has a non-UTF-8 name"),
        )?;
        let relative = path
            .strip_prefix(profile)
            .map_err(|_| ApplicationError::InvalidSetupValue("Classic resource escaped profile"))?
            .to_path_buf();
        let (key, kind, format) = if name.eq_ignore_ascii_case("SPITFIRE.HLP") {
            (
                "SPITFIRE.HLP".to_owned(),
                ProfileResourceKind::Help,
                ProfileFormat::SpitfireHelp,
            )
        } else {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or(ApplicationError::InvalidSetupValue(
                    "Classic display has a non-UTF-8 stem",
                ))?
                .to_ascii_uppercase();
            let format = match path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_uppercase)
                .as_deref()
            {
                Some("BBS") => ProfileFormat::Bbs,
                Some("CLR") => ProfileFormat::Clr,
                _ => {
                    return Err(ApplicationError::InvalidSetupValue(
                        "Classic display has an unsupported extension",
                    ));
                }
            };
            let kind = if stem.starts_with("MAIN")
                || stem.starts_with("MSG")
                || stem.starts_with("FILE")
                || stem.starts_with("SOP")
            {
                ProfileResourceKind::MenuArtwork
            } else {
                ProfileResourceKind::Display
            };
            (stem, kind, format)
        };
        let provenance_id = format!("classic-asset-{:03}", index + 1);
        let is_help = name.eq_ignore_ascii_case("SPITFIRE.HLP");
        provenance.push(ProvenanceRecord {
            id: provenance_id.clone(),
            kind: ProvenanceKind::HistoricalInspired,
            creator: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            rightsholder: "Craig Daters and SPITFIRE NG contributors".to_owned(),
            source: if is_help {
                format!(
                    "Independent SPITFIRE NG help text for {} informed by the M034 evidence map and current action authority",
                    relative.display()
                )
            } else {
                format!(
                    "Independent SPITFIRE NG composition for {} informed by the M036 fidelity review and current action authority",
                    relative.display()
                )
            },
            source_hash: None,
            license: "MIT OR Apache-2.0".to_owned(),
            redistribution: Redistribution::Allowed,
            modifications: Some(if is_help {
                "M035 independent wording retained unchanged in M036; no historical prose copied."
                    .to_owned()
            } else {
                "M036 independent fidelity refinement; no historical resource or screenshot bytes copied."
                    .to_owned()
            }),
            evidence: Some(if is_help {
                "docs/classic-presentation-profile.md".to_owned()
            } else {
                "docs/research/m036-classic-fidelity-review.md".to_owned()
            }),
        });
        resources.push(ProfileResourceRecord {
            key,
            kind,
            format,
            path: relative,
            bytes: bytes.len() as u64,
            sha256: hex_digest(&bytes),
            provenance: provenance_id,
        });
    }
    let descriptor = ProfileDescriptor {
        format_version: PROFILE_FORMAT_VERSION,
        id: CLASSIC_PROFILE_ID.to_owned(),
        version: CLASSIC_PROFILE_VERSION.to_owned(),
        display_name: "Classic SPITFIRE-Inspired".to_owned(),
        description: "An independently authored 80-column CP437/ANSI presentation for SPITFIRE NG."
            .to_owned(),
        resource_api_version: RESOURCE_API_VERSION,
        engine: EngineCompatibility {
            minimum: "0.1.0".to_owned(),
            maximum_exclusive: "0.2.0".to_owned(),
        },
        compatibility_target: "SPITFIRE NG Classic caller presentation".to_owned(),
        supported_formats: vec![
            ProfileFormat::Bbs,
            ProfileFormat::Clr,
            ProfileFormat::SpitfireHelp,
        ],
        fallback_policy: FallbackPolicy::BaseThenBuiltIn,
        provenance,
        resources,
    };
    let encoded = toml::to_string_pretty(&descriptor)
        .map_err(|_| ApplicationError::InvalidSetupValue("could not serialize Classic profile"))?;
    write_new(&profile.join(PROFILE_DESCRIPTOR), encoded.as_bytes())
}

fn classic_help_bytes() -> Result<Vec<u8>, ApplicationError> {
    let blank = HelpRecord::from_lines([b"", b"", b"", b"", b"", b""]).map_err(|source| {
        ApplicationError::HelpResource {
            path: PathBuf::from("Classic SPITFIRE.HLP"),
            source,
        }
    })?;
    let mut records = vec![blank; sf_legacy::HELP_RECORD_COUNT];
    for (number, title, detail) in [
        (
            1,
            "Page the Sysop",
            "Send one live-chat request to the configured Sysop.",
        ),
        (
            2,
            "Goodbye & Log Off",
            "Close this SPITFIRE NG session cleanly.",
        ),
        (
            3,
            "Xpert Mode",
            "Toggle full resource-authored menu displays.",
        ),
        (
            4,
            "Sysop Utilities",
            "Open the security-controlled Sysop Menu.",
        ),
        (
            7,
            "Change File Area",
            "Select a file area your caller may access.",
        ),
        (
            8,
            "List Files",
            "List the authorized files in the current area.",
        ),
        (
            10,
            "Download a File",
            "Download one catalog file with your selected protocol.",
        ),
        (
            11,
            "Read a Text File",
            "Safely previews a bounded text file.",
        ),
        (
            12,
            "Message Section",
            "Move from the File Menu to the Message Menu.",
        ),
        (
            13,
            "Quit to Main",
            "Return from the File Menu to the Main Menu.",
        ),
        (
            14,
            "View a File Archive",
            "Lists bounded ZIP archive metadata without extraction.",
        ),
        (
            15,
            "New Files",
            "List files from your checkpoint or a full-year date.",
        ),
        (
            16,
            "Search Descriptions",
            "Find files whose descriptions contain your search words.",
        ),
        (
            17,
            "Find a File",
            "Find visible files by a bounded wildcard filename.",
        ),
        (
            18,
            "Help With Commands",
            "Select one currently visible command for contextual help.",
        ),
        (
            20,
            "Upload a File",
            "Upload safely; catalog only after integrity checks.",
        ),
        (
            21,
            "Message Section",
            "Open the SPITFIRE Message Menu from Main.",
        ),
        (22, "File Section", "Open the SPITFIRE File Menu from Main."),
        (
            24,
            "Comment to Sysop",
            "Leave a private persisted message for the configured Sysop.",
        ),
        (
            30,
            "Your Statistics",
            "Show current caller, message, file, and terminal statistics.",
        ),
        (
            34,
            "Browse Messages",
            "List messages visible in the current conference.",
        ),
        (
            35,
            "Change Conference",
            "Select a message conference your caller may access.",
        ),
        (
            36,
            "Read Messages",
            "Read visible messages and safely update last-read state.",
        ),
        (
            37,
            "Specific Caller Messages",
            "Find public messages to or from an active caller.",
        ),
        (
            38,
            "Enter New Message",
            "Post an authorized public or private message.",
        ),
        (
            39,
            "Your Messages",
            "Show live waiting, received, and sent message state.",
        ),
        (
            40,
            "Message Text Search",
            "Find visible bodies containing one to six exact terms.",
        ),
        (
            41,
            "File Section",
            "Move from the Message Menu to the File Menu.",
        ),
        (
            42,
            "Quit to Main",
            "Return from the Message Menu to the Main Menu.",
        ),
        (
            53,
            "Alter Conference Queue",
            "Choose accessible conferences for queued message scans.",
        ),
    ] {
        records[number - 1] = HelpRecord::from_lines([
            title.as_bytes(),
            b";",
            detail.as_bytes(),
            b"\\",
            b"\\",
            b"\\",
        ])
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("Classic SPITFIRE.HLP"),
            source,
        })?;
    }
    HelpFile::new(records)
        .map(|help| help.to_bytes())
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("Classic SPITFIRE.HLP"),
            source,
        })
}

fn synthetic_help_bytes() -> Result<Vec<u8>, ApplicationError> {
    let blank = HelpRecord::from_lines([b"", b"", b"", b"", b"", b""]).map_err(|source| {
        ApplicationError::HelpResource {
            path: PathBuf::from("synthetic SPITFIRE.HLP"),
            source,
        }
    })?;
    let mut records = vec![blank; sf_legacy::HELP_RECORD_COUNT];
    for (number, title, detail) in [
        (1, "Page Sysop", "Requests a live text chat with the Sysop."),
        (
            2,
            "Goodbye & Log Off",
            "Ends this SPITFIRE session cleanly.",
        ),
        (3, "Xpert Level", "Toggles full menu displays on and off."),
        (
            4,
            "Go To Sysop Section",
            "Enters the security-controlled Sysop Utilities menu.",
        ),
        (7, "Change File Area", "Selects an accessible file area."),
        (8, "List Files", "Lists files in the current file area."),
        (
            10,
            "Download A File",
            "Downloads an authorized catalog file.",
        ),
        (
            11,
            "Read A Text File",
            "Safely previews a bounded text file.",
        ),
        (
            12,
            "Message Section",
            "Moves from the File Menu to Messages.",
        ),
        (
            13,
            "Quit To Main Menu",
            "Returns from the File Menu to Main.",
        ),
        (
            14,
            "View A File Archive",
            "Lists bounded ZIP archive metadata without extraction.",
        ),
        (
            15,
            "New Files",
            "Lists authorized files added since your last call.",
        ),
        (
            16,
            "File Description Search",
            "Searches authorized file descriptions.",
        ),
        (
            17,
            "Find A File",
            "Finds authorized files by wildcard name.",
        ),
        (
            18,
            "HELP With Commands",
            "Select a displayed command for help.",
        ),
        (
            20,
            "Upload A File",
            "Uploads into safe per-session staging.",
        ),
        (
            21,
            "Message Section",
            "Moves from Main to the Message Menu.",
        ),
        (22, "File Section", "Moves from Main to the File Menu."),
        (
            24,
            "Comment To Sysop",
            "Leaves a private persisted message for the Sysop.",
        ),
        (
            30,
            "Your Statistics",
            "Shows your persisted caller and transfer statistics.",
        ),
        (
            34,
            "Browse Messages",
            "Lists messages visible in the current conference.",
        ),
        (
            35,
            "Change Message Conference",
            "Selects an accessible message conference.",
        ),
        (
            36,
            "Read Messages",
            "Reads visible messages and updates last-read state.",
        ),
        (
            37,
            "Specific Caller Messages",
            "Finds public messages to or from an active caller.",
        ),
        (38, "Enter A Message", "Posts a public or private message."),
        (
            39,
            "Your Messages",
            "Shows received/sent counts and opens either message list.",
        ),
        (
            40,
            "Message Text Search",
            "Finds visible bodies containing one to six exact terms.",
        ),
        (41, "File Section", "Moves from Messages to the File Menu."),
        (42, "Quit To Main Menu", "Returns from Messages to Main."),
        (
            53,
            "Alter Conference Queue",
            "Changes the accessible conferences used by queued scans.",
        ),
    ] {
        records[number - 1] = HelpRecord::from_lines([
            title.as_bytes(),
            b";",
            detail.as_bytes(),
            b"\\",
            b"\\",
            b"\\",
        ])
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("synthetic SPITFIRE.HLP"),
            source,
        })?;
    }
    HelpFile::new(records)
        .map(|help| help.to_bytes())
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("synthetic SPITFIRE.HLP"),
            source,
        })
}

fn minimal_help_bytes() -> Result<Vec<u8>, ApplicationError> {
    let blank = HelpRecord::from_lines([b"", b"", b"", b"", b"", b""]).map_err(|source| {
        ApplicationError::HelpResource {
            path: PathBuf::from("Minimal SPITFIRE.HLP"),
            source,
        }
    })?;
    let mut records = vec![blank; sf_legacy::HELP_RECORD_COUNT];
    for (number, title, detail) in [
        (1, "Page Sysop", "Ask the Sysop for a live text chat."),
        (2, "Goodbye", "End this session safely."),
        (3, "Expert mode", "Show or hide full menu text."),
        (4, "Sysop utilities", "Open the protected Sysop menu."),
        (7, "Change file area", "Choose a file area you may access."),
        (8, "List files", "List files in the current area."),
        (10, "Download", "Receive an authorized catalog file."),
        (12, "Messages", "Open the Message menu."),
        (13, "Main menu", "Return to the Main menu."),
        (15, "New files", "List files added since your last call."),
        (
            16,
            "Search descriptions",
            "Search visible file descriptions.",
        ),
        (17, "Find file", "Find a visible file by wildcard name."),
        (18, "Help", "Choose a visible command for more help."),
        (20, "Upload", "Send a file to safe session staging."),
        (21, "Messages", "Open the Message menu."),
        (22, "Files", "Open the File menu."),
        (
            24,
            "Comment to Sysop",
            "Send a private message to the Sysop.",
        ),
        (30, "Statistics", "Show your caller and transfer totals."),
        (34, "Browse messages", "List messages you may read."),
        (
            35,
            "Change conference",
            "Choose a conference you may access.",
        ),
        (36, "Read messages", "Read visible messages."),
        (38, "Enter message", "Post a public or private message."),
        (39, "Your messages", "Open messages sent by or to you."),
        (
            40,
            "Conference queue",
            "Choose conferences for queued scans.",
        ),
        (41, "Files", "Open the File menu."),
        (42, "Main menu", "Return to the Main menu."),
    ] {
        records[number - 1] = HelpRecord::from_lines([
            title.as_bytes(),
            b";",
            detail.as_bytes(),
            b"\\",
            b"\\",
            b"\\",
        ])
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("Minimal SPITFIRE.HLP"),
            source,
        })?;
    }
    HelpFile::new(records)
        .map(|help| help.to_bytes())
        .map_err(|source| ApplicationError::HelpResource {
            path: PathBuf::from("Minimal SPITFIRE.HLP"),
            source,
        })
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), ApplicationError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ApplicationError::CreateFixtureFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(contents)
        .map_err(|source| ApplicationError::WriteFixtureFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all()
        .map_err(|source| ApplicationError::WriteFixtureFile {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_deterministic_synthetic_fixture_board() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("var/fixture-board");
        let report = initialize_fixture_board(&root).unwrap();

        assert_eq!(report.schema_version, sf_core::SCHEMA_VERSION);
        assert!(report.config_path.is_file());
        assert!(report.database_path.is_file());
        for directory in ["system", "work", "display", "message", "external"] {
            assert!(root.join(directory).is_dir());
        }
        let notice = fs::read_to_string(
            root.join("system/presentation-profiles/modern-ng")
                .join(DISPLAY_NOTICE_FILE),
        )
        .unwrap();
        assert!(notice.contains("not Buffalo Creek historical assets"));
        let minimal = root.join("system/presentation-profiles/minimal-terminal");
        let minimal_notice = fs::read_to_string(minimal.join(DISPLAY_NOTICE_FILE)).unwrap();
        assert!(minimal_notice.contains("Synthetic Minimal Terminal profile"));
        let descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(minimal.join(PROFILE_DESCRIPTOR)).unwrap()).unwrap();
        assert_eq!(descriptor.id, MINIMAL_PROFILE_ID);
        assert_eq!(descriptor.version, crate::MINIMAL_PROFILE_VERSION);
        assert_eq!(
            descriptor.supported_formats,
            vec![ProfileFormat::Bbs, ProfileFormat::SpitfireHelp]
        );

        let classic = root.join("system/presentation-profiles/classic-spitfire");
        let classic_notice = fs::read_to_string(classic.join(DISPLAY_NOTICE_FILE)).unwrap();
        assert!(classic_notice.contains("no historical bytes"));
        let descriptor: ProfileDescriptor =
            toml::from_str(&fs::read_to_string(classic.join(PROFILE_DESCRIPTOR)).unwrap()).unwrap();
        assert_eq!(descriptor.id, CLASSIC_PROFILE_ID);
        assert_eq!(descriptor.version, CLASSIC_PROFILE_VERSION);
        assert_eq!(descriptor.resources.len(), 68);
        assert_eq!(descriptor.provenance.len(), 68);
        assert_eq!(
            descriptor
                .resources
                .iter()
                .filter(|resource| resource.format == ProfileFormat::Bbs)
                .count(),
            34
        );
        assert_eq!(
            descriptor
                .resources
                .iter()
                .filter(|resource| resource.format == ProfileFormat::Clr)
                .count(),
            33
        );
        assert!(descriptor
            .provenance
            .iter()
            .all(|record| record.kind == ProvenanceKind::HistoricalInspired
                && record.redistribution == Redistribution::Allowed
                && record.modifications.is_some()));
        assert!(classic.join("README.md").is_file());
        assert!(classic.join("LICENSES/ASSET-LICENSE.txt").is_file());

        let config_text = fs::read_to_string(report.config_path).unwrap();
        assert_eq!(
            RuntimeConfig::from_toml(&config_text).unwrap(),
            RuntimeConfig::synthetic_fixture()
        );
    }

    #[test]
    fn packaged_resources_declare_the_project_license_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        initialize_fixture_board(&root).unwrap();

        for profile in [MODERN_PROFILE_ID, MINIMAL_PROFILE_ID, CLASSIC_PROFILE_ID] {
            let package = root.join("system/presentation-profiles").join(profile);
            let descriptor: ProfileDescriptor =
                toml::from_str(&fs::read_to_string(package.join(PROFILE_DESCRIPTOR)).unwrap())
                    .unwrap();
            assert!(descriptor
                .provenance
                .iter()
                .all(|record| record.license == "MIT OR Apache-2.0"));
            let notice = fs::read_to_string(package.join("LICENSES/ASSET-LICENSE.txt")).unwrap();
            assert!(notice.contains("MIT OR Apache-2.0"));
            assert!(
                notice.contains("original copyright") || notice.contains("original copyrights")
            );
        }

        let language = root.join("system/language-packs/en-US");
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(language.join("language.toml")).unwrap()).unwrap();
        let provenance = manifest
            .get("provenance")
            .and_then(toml::Value::as_array)
            .unwrap();
        assert!(provenance.iter().all(|record| {
            record.get("license").and_then(toml::Value::as_str) == Some("MIT OR Apache-2.0")
        }));
        let notice = fs::read_to_string(language.join("LICENSES/ASSET-LICENSE.txt")).unwrap();
        assert!(notice.contains("MIT OR Apache-2.0"));
        assert!(notice.contains("No Buffalo Creek Software resource bytes are included"));
    }

    #[test]
    fn refuses_to_overwrite_an_existing_fixture_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("fixture-board");
        fs::create_dir(&root).unwrap();
        let sentinel = root.join("keep.txt");
        fs::write(&sentinel, b"keep").unwrap();

        assert!(matches!(
            initialize_fixture_board(&root),
            Err(ApplicationError::FixtureExists(path)) if path == root
        ));
        assert_eq!(fs::read(sentinel).unwrap(), b"keep");
    }
}

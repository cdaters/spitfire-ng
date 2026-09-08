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

use super::*;
use crate::{FileAccessMode, FileAreaDefinition, LogicalPaths, RuntimeConfig, SecurityLevel};
use std::io::Cursor;

struct Mock(ScanResult);
impl Scanner for Mock {
    fn scan(&self, _: &mut dyn Read) -> scanner::ScanReport {
        scanner::ScanReport::new("synthetic-test", self.0.clone())
    }
}
pub(crate) fn board() -> (tempfile::TempDir, RuntimeDatabase, FileStorage, FileArea) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = LogicalPaths::resolve(
        tmp.path(),
        &RuntimeConfig::synthetic_fixture().validate().unwrap(),
    )
    .unwrap();
    paths.create_directories().unwrap();
    let mut db = RuntimeDatabase::open(paths.database()).unwrap();
    db.migrate().unwrap();
    let area = db
        .create_file_area(&FileAreaDefinition {
            number: 1,
            name: "Synthetic files".into(),
            description: "Test".into(),
            storage_key: "native".into(),
            access_mode: FileAccessMode::AtLeast,
            read_security: SecurityLevel::new(1).unwrap(),
            upload_security: SecurityLevel::new(1).unwrap(),
            preview: false,
            no_charge: false,
            maximum_upload_bytes: 1024 * 1024,
            privileged_security_levels: vec![],
        })
        .unwrap();
    let store = FileStorage::new(&paths).unwrap();
    (tmp, db, store, area)
}
fn import(
    db: &mut RuntimeDatabase,
    store: &FileStorage,
    area: &FileArea,
    name: &str,
    bytes: &[u8],
    scanner: Option<&dyn Scanner>,
) -> Result<ImportResult> {
    store.import_file(
        db,
        FileAdminActor::LocalOperator,
        area,
        name,
        "Uploader description",
        &mut Cursor::new(bytes),
        "operator",
        scanner,
    )
}
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        z.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap().into_inner()
}
#[test]
fn content_identity_collisions_originals_and_reference_deletion() {
    let (_t, mut db, store, area) = board();
    let a = import(&mut db, &store, &area, "ONE.TXT", b"hello", None).unwrap();
    assert_eq!(
        a.file.sha256,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    let b = import(&mut db, &store, &area, "TWO.TXT", b"hello", None).unwrap();
    assert!(b.duplicate_content);
    assert_ne!(a.file.id, b.file.id);
    assert!(
        import(&mut db, &store, &area, "ONE.TXT", b"hello", None)
            .unwrap()
            .existing
    );
    assert!(import(&mut db, &store, &area, "ONE.TXT", b"changed", None)
        .unwrap_err()
        .to_string()
        .contains("collision"));
    assert_eq!(db.file_count(area.id).unwrap(), 2);
    fs::remove_file(store.ensure_area(&area).unwrap().join("ONE.TXT")).unwrap();
    assert!(store.open_content(&a.file.sha256, 5).is_ok());
    assert!(store.open_download(&area, &b.file).is_ok());
    assert!(db
        .connection
        .execute(
            "UPDATE files SET sha256=lower(hex(randomblob(32))) WHERE file_id=?1",
            [b.file.id.get()]
        )
        .is_err());
}
#[test]
fn required_scan_quarantine_approval_rescan_and_legacy_review_fence() {
    let (_t, mut db, store, area) = board();
    db.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        area.id,
        &SafetyPolicy::default(),
    )
    .unwrap();
    let a = import(&mut db, &store, &area, "SAFE.TXT", b"safe fixture", None).unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
    assert_eq!(db.file_count(area.id).unwrap(), 0);
    assert!(db
        .review_file_admission(FileAdminActor::LocalOperator, a.file.id, true)
        .is_err());
    assert!(db
        .connection
        .execute(
            "UPDATE files SET lifecycle='active' WHERE file_id=?1",
            [a.file.id.get()]
        )
        .is_err());
    let clean = Mock(ScanResult::Clean);
    assert_eq!(
        store
            .rescan_file(
                &mut db,
                FileAdminActor::LocalOperator,
                a.file.id,
                Some(&clean)
            )
            .unwrap()
            .status,
        AdmissionStatus::PendingApproval
    );
    db.review_file_admission(FileAdminActor::LocalOperator, a.file.id, true)
        .unwrap();
    assert_eq!(db.file_count(area.id).unwrap(), 1);
    let malware = Mock(ScanResult::MalwareDetected);
    assert_eq!(
        store
            .rescan_file(
                &mut db,
                FileAdminActor::LocalOperator,
                a.file.id,
                Some(&malware)
            )
            .unwrap()
            .status,
        AdmissionStatus::Quarantined
    );
    db.review_file_admission(FileAdminActor::LocalOperator, a.file.id, false)
        .unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Rejected
    );
    assert!(store
        .open_content(&a.file.sha256, a.file.size_bytes)
        .is_ok());
}
#[test]
fn diz_is_suggestion_safe_text_and_original_archive_is_unchanged() {
    let (_t, mut db, store, area) = board();
    let bytes = zip(&[
        ("file_id.diz", b"Description\nSecond line\x1b[31m"),
        ("README.TXT", b"fixture"),
    ]);
    let a = import(&mut db, &store, &area, "ARCHIVE.ZIP", &bytes, None).unwrap();
    let admission = db.file_admission(a.file.id).unwrap().unwrap();
    assert_eq!(admission.status, AdmissionStatus::Published);
    let diz = admission.report.unwrap().diz.unwrap();
    assert!(diz.original.contains(&27));
    assert!(!diz.suggestion.contains('\u{1b}'));
    assert_eq!(a.file.description, "Uploader description");
    db.use_file_description(
        FileAdminActor::LocalOperator,
        a.file.id,
        Some("Reviewed description"),
    )
    .unwrap();
    let mut copy = vec![];
    store
        .open_content(&a.file.sha256, a.file.size_bytes)
        .unwrap()
        .read_to_end(&mut copy)
        .unwrap();
    assert_eq!(copy, bytes);
}
#[test]
fn archive_traversal_links_limits_encryption_recognition_and_nested_bounds() {
    let (_t, mut db, store, area) = board();
    for (i, name) in [
        "../escape",
        "/absolute",
        "C:/drive",
        "dir\\escape",
        "x/../y",
    ]
    .iter()
    .enumerate()
    {
        let bytes = zip(&[(name, b"synthetic")]);
        let a = import(&mut db, &store, &area, &format!("BAD{i}.ZIP"), &bytes, None).unwrap();
        assert_eq!(
            db.file_admission(a.file.id).unwrap().unwrap().status,
            AdmissionStatus::Quarantined
        );
    }
    let mut policy = SafetyPolicy::legacy();
    policy.archives.nesting = 0;
    policy.archives.members = 2;
    db.set_file_safety_policy(FileAdminActor::LocalOperator, area.id, &policy)
        .unwrap();
    let nested = zip(&[("inner.zip", &zip(&[("x", b"a")]))]);
    let a = import(&mut db, &store, &area, "NEST.ZIP", &nested, None).unwrap();
    assert_eq!(
        db.file_admission(a.file.id)
            .unwrap()
            .unwrap()
            .report
            .unwrap()
            .archive_error
            .as_deref(),
        Some("nesting-limit")
    );
    let excess = zip(&[("a", b"a"), ("b", b"b"), ("c", b"c")]);
    let a = import(&mut db, &store, &area, "MANY.ZIP", &excess, None).unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
    assert_eq!(
        archive::detect(b"MZthis-is-not-a-jpeg"),
        archive::MediaType::Executable
    );
    assert_eq!(
        archive::detect(b"7z\xbc\xaf\x27\x1c"),
        archive::MediaType::SevenZip
    );
    assert_eq!(archive::detect(b"Rar!\x1a\x07"), archive::MediaType::Rar);
    let a = import(&mut db, &store, &area, "FAKE.ZIP", b"not an archive", None).unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
}
#[test]
fn tar_tgz_inspection_and_link_rejection() {
    let (_t, mut db, store, area) = board();
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "FILE_ID.DIZ", Cursor::new(b"hello"))
        .unwrap();
    let bytes = tar.into_inner().unwrap();
    let a = import(&mut db, &store, &area, "OK.TAR", &bytes, None).unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Published
    );
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&bytes).unwrap();
    let a = import(
        &mut db,
        &store,
        &area,
        "OK.TGZ",
        &gzip.finish().unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        db.file_admission(a.file.id)
            .unwrap()
            .unwrap()
            .report
            .unwrap()
            .diz
            .unwrap()
            .suggestion,
        "hello"
    );
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_mode(0o777);
    header.set_cksum();
    tar.append_link(&mut header, "link", "../../outside")
        .unwrap();
    let a = import(
        &mut db,
        &store,
        &area,
        "LINK.TAR",
        &tar.into_inner().unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
}
#[test]
fn restart_restore_and_missing_corrupt_payloads_fail_visibly() {
    let (t, mut db, store, area) = board();
    db.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        area.id,
        &SafetyPolicy::default(),
    )
    .unwrap();
    let a = import(&mut db, &store, &area, "HELD.TXT", b"evidence", None).unwrap();
    db.admission_status(a.file.id, AdmissionStatus::Inspecting, "synthetic-crash")
        .unwrap();
    let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
    let paths = LogicalPaths::resolve(t.path(), &config).unwrap();
    drop(db);
    let db = RuntimeDatabase::open(paths.database()).unwrap();
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Inspecting
    );
    fs::remove_file(store.content_root().unwrap().join(&a.file.sha256)).unwrap();
    assert!(store
        .open_content(&a.file.sha256, a.file.size_bytes)
        .is_err());
    assert!(store.rebuild_restored_content(&db).is_err());
    assert_eq!(
        db.file_admission(a.file.id).unwrap().unwrap().status,
        AdmissionStatus::Inspecting
    );
}

#[test]
fn archive_expansion_duplicate_paths_encryption_diz_and_unknown_formats() {
    let (_t, mut db, store, area) = board();
    let mut encrypted = zip(&[("READ.TXT", b"synthetic")]);
    for signature in [b"PK\x03\x04", b"PK\x01\x02"] {
        let offset = encrypted.windows(4).position(|w| w == signature).unwrap();
        let flag = offset + if signature == b"PK\x03\x04" { 6 } else { 8 };
        encrypted[flag] |= 1;
    }
    for (name, bytes) in [
        ("ENCRYPT.ZIP", encrypted),
        ("CONFLICT.ZIP", zip(&[("a", b"file"), ("a/b", b"conflict")])),
        ("DIZ.ZIP", zip(&[("FILE_ID.DIZ", b"bad\0text")])),
        ("RAR.RAR", b"Rar!\x1a\x07\0synthetic".to_vec()),
    ] {
        let file = import(&mut db, &store, &area, name, &bytes, None)
            .unwrap()
            .file;
        assert_eq!(
            db.file_admission(file.id).unwrap().unwrap().status,
            AdmissionStatus::Quarantined
        );
    }
    let mut policy = SafetyPolicy::legacy();
    policy.archives.member_bytes = 16;
    db.set_file_safety_policy(FileAdminActor::LocalOperator, area.id, &policy)
        .unwrap();
    let file = import(
        &mut db,
        &store,
        &area,
        "EXPAND.ZIP",
        &zip(&[("TOO.LARGE", &[0; 32])]),
        None,
    )
    .unwrap()
    .file;
    assert_eq!(
        db.file_admission(file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
}
#[test]
fn scanner_outcomes_and_missing_inspection_do_not_bypass_required_policy() {
    let (_t, mut db, store, area) = board();
    db.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        area.id,
        &SafetyPolicy::default(),
    )
    .unwrap();
    for (i, result) in [
        ScanResult::ScannerError,
        ScanResult::ScannerUnavailable,
        ScanResult::Unsupported,
        ScanResult::SkippedByPolicy,
        ScanResult::Suspicious,
    ]
    .into_iter()
    .enumerate()
    {
        let file = import(
            &mut db,
            &store,
            &area,
            &format!("SCAN{i}.TXT"),
            b"harmless",
            Some(&Mock(result)),
        )
        .unwrap()
        .file;
        assert_eq!(
            db.file_admission(file.id).unwrap().unwrap().status,
            AdmissionStatus::Quarantined
        );
    }
    // Crash between catalog commit and validation insertion remains unpublished.
    let file = import(
        &mut db,
        &store,
        &area,
        "INCOMPLETE.TXT",
        b"incomplete admission",
        None,
    )
    .unwrap()
    .file;
    db.connection
        .execute(
            "DELETE FROM file_validation WHERE file_id=?1",
            [file.id.get()],
        )
        .unwrap();
    assert!(db
        .review_pending_file(
            FileAdminActor::LocalOperator,
            file.id,
            file.state_version,
            true
        )
        .is_err());
    assert!(db
        .connection
        .execute(
            "UPDATE files SET lifecycle='active' WHERE file_id=?1",
            [file.id.get()]
        )
        .is_err());
    // A legacy atomic adapter cannot publish around an explicitly configured policy.
    let current = db.load_area_by_id(area.id).unwrap().unwrap();
    assert!(db
        .add_managed_file_committing(
            &store,
            FileAdminActor::LocalOperator,
            area.id,
            current.state_version,
            "BYPASS.TXT",
            "synthetic",
            b"safe",
            |_, _| panic!("must not reach publication callback")
        )
        .is_err());
}
#[test]
fn required_policy_and_quarantine_survive_native_snapshot_validation() {
    let (t, mut db, store, area) = board();
    db.ensure_board_identity(&crate::BoardIdentity::new("Synthetic Files", "Sysop").unwrap())
        .unwrap();
    db.set_file_safety_policy(
        FileAdminActor::LocalOperator,
        area.id,
        &SafetyPolicy::default(),
    )
    .unwrap();
    let file = import(
        &mut db,
        &store,
        &area,
        "EVIDENCE.TXT",
        b"retained evidence",
        None,
    )
    .unwrap()
    .file;
    db.validate_current_snapshot().unwrap();
    let path = t.path().join("snapshot.sqlite3");
    db.backup_to(&path).unwrap();
    let restored = RuntimeDatabase::open(&path).unwrap();
    assert_eq!(
        restored.file_admission(file.id).unwrap().unwrap().status,
        AdmissionStatus::Quarantined
    );
    restored.validate_current_snapshot().unwrap();
}

#[test]
fn remote_readvertisement_cannot_undo_local_rejection() {
    let (_temp, mut db, store, area) = board();
    let imported = import(&mut db, &store, &area, "REJECTED.TXT", b"harmless", None).unwrap();
    db.review_file_admission(FileAdminActor::LocalOperator, imported.file.id, false)
        .unwrap();
    let repeated = store
        .import_file(
            &mut db,
            FileAdminActor::LocalOperator,
            &area,
            "REJECTED.TXT",
            "remote",
            &mut std::io::Cursor::new(b"harmless"),
            "circuitnet",
            None,
        )
        .unwrap();
    assert!(repeated.existing);
    assert_eq!(
        db.file_admission(imported.file.id).unwrap().unwrap().status,
        AdmissionStatus::Rejected
    );
    assert_eq!(db.file_count(area.id).unwrap(), 0);
    store
        .rescan_file(
            &mut db,
            FileAdminActor::LocalOperator,
            imported.file.id,
            None,
        )
        .unwrap();
    assert_eq!(db.file_count(area.id).unwrap(), 1);
}

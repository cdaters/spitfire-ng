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

//! Public independent vectors exercised by the production codec.
use sf_net::circuitnet::{self as cn, catalog, control, files, transport};
use std::io::Cursor;

fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tools/circuitnet-conformance/vectors/valid.json"
    ))
    .unwrap()
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn independently_authored_golden_bytes_hashes_and_signatures() {
    let v = vectors();
    let batch: cn::Batch = serde_json::from_value(v["batch"].clone()).unwrap();
    assert_eq!(
        batch.encode().unwrap(),
        unhex(v["batch_canonical_hex"].as_str().unwrap())
    );
    assert_eq!(
        cn::digest(&batch.encode().unwrap()),
        v["batch_hash"].as_str().unwrap()
    );
    for (m, expected) in batch
        .messages
        .iter()
        .zip(v["message_fingerprints"].as_array().unwrap())
    {
        assert_eq!(m.fingerprint().unwrap(), expected.as_str().unwrap());
    }
    let directed: cn::Batch = serde_json::from_value(v["directed"].clone()).unwrap();
    assert_eq!(
        cn::digest(&directed.encode().unwrap()),
        v["directed_hash"].as_str().unwrap()
    );
    for (r, expected) in v["controls"]
        .as_array()
        .unwrap()
        .iter()
        .zip(v["control_fingerprints"].as_array().unwrap())
    {
        let r: control::Request = serde_json::from_value(r.clone()).unwrap();
        r.validate().unwrap();
        assert_eq!(r.fingerprint().unwrap(), expected.as_str().unwrap());
    }
    let p: files::Publication = serde_json::from_value(v["publication"].clone()).unwrap();
    assert_eq!(
        p.fingerprint().unwrap(),
        v["publication_fingerprint"].as_str().unwrap()
    );
    let pin: catalog::Authority = serde_json::from_value(v["authority"].clone()).unwrap();
    let mut previous = None;
    for (s, raw) in v["catalogs"]
        .as_array()
        .unwrap()
        .iter()
        .zip(v["catalog_canonical_hex"].as_array().unwrap())
    {
        let s: catalog::Signed = serde_json::from_value(s.clone()).unwrap();
        s.verify(&pin).unwrap();
        assert_eq!(s.body.canonical().unwrap(), unhex(raw.as_str().unwrap()));
        assert!(s.follows(previous.as_ref()).unwrap());
        previous = Some(s);
    }
}
#[test]
fn independent_malformed_frame_vectors_fail_closed() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/circuitnet-conformance/vectors/malformed.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let wire = unhex(case["wire_hex"].as_str().unwrap());
        let result = transport::read(&mut Cursor::new(wire), transport::MAX_FRAME);
        assert!(result.is_err(), "accepted {}", case["name"]);
        assert_eq!(
            result.unwrap_err().to_string(),
            case["expected"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

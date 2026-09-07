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
fn header(point: u16) -> PacketHeader {
    PacketHeader {
        profile: PacketProfile::Type2Plus,
        origin: Address::new(10, 100, 1, point).unwrap(),
        destination: Address::new(10, 100, 2, 7).unwrap(),
        created: chrono::NaiveDate::from_ymd_opt(2026, 9, 5)
            .unwrap()
            .and_hms_opt(12, 30, 0)
            .unwrap(),
        password: [0; 8],
        product: [254, 0, 0, 1],
        product_data: [0; 4],
        baud: 0,
        spare: [0; 20],
    }
}
fn message() -> PackedMessage {
    PackedMessage {
        origin: (100, 1),
        destination: (100, 2),
        attributes: 1,
        cost: 0,
        date: format_date(header(0).created).unwrap(),
        to: b"Recipient".to_vec(),
        from: b"Author".to_vec(),
        subject: b"Synthetic".to_vec(),
        text: b"\x01INTL 10:100/2 10:100/1\r\x01FMPT 3\r\x01TOPT 7\rBody\r".to_vec(),
    }
}
#[test]
fn address_domains_canonical_keys_and_hostile_forms() {
    let a: Address = "0010:00100/00001.00003".parse().unwrap();
    assert_eq!(a.to_string(), "10:100/1.3");
    assert_eq!(a.boss().point(), 0);
    for s in [
        "0:1/2",
        "1:0/2",
        "1:1/65536",
        "1:1/2.-1",
        "+1:1/2",
        "1:1/2.1.1",
        "1/2",
        "1:1/2@net",
        " 1:1/2",
    ] {
        assert!(s.parse::<Address>().is_err(), "{s}")
    }
    assert_eq!("1:1/0".parse::<Address>().unwrap().node(), 0);
    let a: Endpoint = "10:100/1.3@TeSt".parse().unwrap();
    assert_eq!(a.to_string(), "10:100/1.3@test");
    assert_ne!(a, "10:100/1.3@other".parse().unwrap());
    let mut keys = std::collections::HashSet::new();
    keys.insert(a);
    keys.insert("10:100/1.3@test".parse().unwrap());
    assert_eq!(keys.len(), 1);
}
#[test]
fn packet_points_auxnet_and_every_truncation() {
    let p = Packet {
        header: header(3),
        messages: vec![message()],
    };
    let bytes = p.encode().unwrap();
    assert_eq!(&bytes[20..22], &[255, 255]);
    assert_eq!(&bytes[38..40], &[100, 0]);
    let d = Packet::decode(&bytes, (10, 10)).unwrap();
    assert_eq!(d.header.origin.point(), 3);
    assert_eq!(d.header.destination.point(), 7);
    assert!(d.encode().unwrap() == bytes);
    for n in 0..bytes.len() {
        assert!(
            Packet::decode(&bytes[..n], (10, 10)).is_err(),
            "truncation {n}"
        )
    }
}
#[test]
fn packet_variants_and_structural_bounds() {
    let mut p = Packet {
        header: header(0),
        messages: vec![message()],
    };
    p.header.profile = PacketProfile::Type2;
    p.header.destination = p.header.destination.boss();
    let mut b = p.encode().unwrap();
    assert_eq!(
        Packet::decode(&b, (10, 10)).unwrap().header.profile,
        PacketProfile::Type2
    );
    b[16] = 2;
    assert!(matches!(Packet::decode(&b, (10, 10)), Err(Error::Profile)));
    assert!(matches!(
        Packet::decode(&vec![0; MAX_PACKET + 1], (10, 10)),
        Err(Error::Limit)
    ));
    p.messages[0].subject = vec![b'x'; 72];
    assert!(p.encode().is_err());
}
#[test]
fn controls_points_and_hidden_unknown_roundtrip() {
    let raw=b"\x01MSGID: 10:100/1.3 00000001\r\x01INTL 10:100/2 10:100/1\r\x01FMPT 3\r\x01TOPT 7\rHello\r\x01EXPERIMENT: retained\rWorld\r";
    let t = Text::parse(raw, Charset::Ascii).unwrap();
    assert_eq!(t.body, "Hello\nWorld");
    let (o, d) = t.addresses(&message(), &header(3)).unwrap();
    assert_eq!(o.point(), 3);
    assert_eq!(d.point(), 7);
    assert!(t.encode().unwrap() == raw);
    assert!(Text::parse(b"\x01FMPT 1\r\x01FMPT 2\rBody", Charset::Ascii).is_err());
    assert!(Text::parse(b"\x01TOPT 65536\rBody", Charset::Ascii).is_err());
}
#[test]
fn echo_footer_quotes_and_point_origin() {
    let raw=b"AREA:SYNTHETIC\r\x01MSGID: 10:100/1.3 00000002\rQuoted text\rSEEN-BY: this is body\rMore body\r--- Test\r * Origin: Synthetic (10:100/1.3)\rSEEN-BY: 100/1 2\r\x01PATH: 100/1\r";
    let t = Text::parse(raw, Charset::Ascii).unwrap();
    assert!(t.body.contains("SEEN-BY: this is body"));
    assert!(!t.body.contains("Origin:"));
    assert_eq!(t.seen_by.len(), 2);
    assert_eq!(t.addresses(&message(), &header(3)).unwrap().0.point(), 3);
    let reparsed = Text::parse(&t.encode().unwrap(), Charset::Ascii).unwrap();
    assert_eq!(t.body, reparsed.body);
    assert_eq!(t.seen_by, reparsed.seen_by);
    assert_eq!(t.path, reparsed.path);
}
#[test]
fn charset_complete_high_byte_roundtrips_and_declarations() {
    for c in [Charset::Cp437, Charset::Cp850, Charset::Cp866] {
        let bytes: Vec<u8> = (128..=255).collect();
        assert_eq!(c.encode(&c.decode(&bytes).unwrap()).unwrap(), bytes);
    }
    assert_eq!(Charset::Cp437.decode(&[0x82]).unwrap(), "é");
    assert_eq!(Charset::Cp866.decode(&[0x80]).unwrap(), "А");
    assert!(Charset::Ascii.decode(&[0x80]).is_err());
    assert!(Charset::Utf8.decode(&[0xff]).is_err());
    assert!(Text::parse(
        b"\x01CHRS: CP866 2\r\x01CHARSET: UTF-8 4\rx",
        Charset::Ascii
    )
    .is_err());
    let t = Text::parse("\u{1}CHRS: UTF-8 2\rПривет".as_bytes(), Charset::Ascii).unwrap();
    assert_eq!(t.body, "Привет");
}
#[test]
fn timestamp_original_offset_pivot_and_invalid_dates() {
    let t = header(0).created;
    assert_eq!(parse_date(&format_date(t).unwrap()).unwrap(), t);
    assert_eq!(utc_offset("-0330").unwrap(), -210);
    assert_eq!(utc_offset("+1300").unwrap(), 780);
    for bad in ["2400", "0199", "-700", "abc", "9999"] {
        assert!(utc_offset(bad).is_err());
    }
    let t = Text::parse(b"\x01TZUTCINFO: -0700\rBody", Charset::Ascii).unwrap();
    assert_eq!(t.offset_minutes, Some(-420));
    let mut date = format_date(header(0).created).unwrap();
    date[..2].copy_from_slice(b"99");
    assert!(parse_date(&date).is_err());
}
#[test]
fn hostile_control_count_and_line_limit() {
    let b = b"\x01UNKNOWN: x\r".repeat(MAX_CONTROLS + 1);
    assert!(matches!(Text::parse(&b, Charset::Ascii), Err(Error::Limit)));
    let b = [b"\x01UNKNOWN: ".as_slice(), &vec![b'x'; MAX_CONTROL_LINE]].concat();
    assert!(matches!(Text::parse(&b, Charset::Ascii), Err(Error::Limit)));
}
#[test]
fn directory_crc_point_boss_and_combined() {
    use directory::*;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
    let records=b"Zone,10,Zone,Here,Operator,-Unpublished-,300\r\nHost,100,Net,Here,Operator,-Unpublished-,300\r\n,1,Node,Here,Operator,-Unpublished-,300,INA:peer.invalid,IBN\r\nPoint,3,Point,Here,Operator,-Unpublished-,300\r\n";
    let bytes = [
        format!(";A Synthetic Day number 248 : {:05}\r\n", crc16(records)).as_bytes(),
        records,
    ]
    .concat();
    let p = DirectoryProfile {
        format: DirectoryFormat::Combined,
        charset: Charset::Ascii,
        date,
        default_zone: 10,
        require_crc: true,
    };
    let c = parse(&bytes, &p).unwrap();
    assert_eq!(
        c.entries.last().unwrap().address,
        "10:100/1.3".parse().unwrap()
    );
    assert!(c.issues.is_empty());
    let mut bad = bytes.clone();
    let last = bad.len() - 4;
    bad[last] = b'9';
    assert!(parse(&bad, &p).is_err());
    let b = b";A synthetic\r\nBoss,10:100/1\r\n,3,Point,Here,Operator,-Unpublished-,300\r\n";
    let c = parse(
        b,
        &DirectoryProfile {
            format: DirectoryFormat::Boss,
            require_crc: false,
            ..p
        },
    )
    .unwrap();
    assert_eq!(c.entries[0].address.point(), 3);
}
#[test]
fn directory_duplicates_and_no_partial_context_guess() {
    use directory::*;
    let p = DirectoryProfile {
        format: DirectoryFormat::Boss,
        charset: Charset::Cp866,
        date: chrono::NaiveDate::from_ymd_opt(2026, 9, 5).unwrap(),
        default_zone: 10,
        require_crc: false,
    };
    let b=b";A synthetic\nBoss,10:100/1\n,3,Point,Here,Operator,-Unpublished-,300\n,3,Other,Here,Operator,-Unpublished-,300\nBoss,garbage\n,4,Lost,Here,Operator,-Unpublished-,300\n";
    let c = parse(b, &p).unwrap();
    assert_eq!(c.entries.len(), 1);
    assert_eq!(c.issues.len(), 3);
    assert!(parse(&vec![b'\n'; 100_002], &p).is_err());
}

#[test]
fn known_control_delimiters_and_generated_bounds_fail_closed() {
    for bytes in [
        b"\x01INTL: 10:100/1 10:100/2\r".as_slice(),
        b"\x01MSGID 10:100/2 00000001\r",
        b"\x01FMPT: 3\r",
    ] {
        assert!(Text::parse(bytes, Charset::Ascii).is_err());
    }
    let mut t = Text::plain("Visible", Charset::Ascii).unwrap();
    for _ in 0..129 {
        t.add_control("X-UNKNOWN: bounded");
    }
    assert!(t.encode().is_err());
    assert!(Text::plain("\x01MSGID: injected", Charset::Ascii).is_err());
}
#[test]
fn rescanned_wire_forms_preserve_bytes_without_relaxing_endpoints() {
    for marker in ["90:100/1", "90:100/1@interop", "90:100/1.4"] {
        let bytes = format!("\x01RESCANNED {marker}\rBody\r").into_bytes();
        let text = Text::parse(&bytes, Charset::Utf8).unwrap();
        assert_eq!(text.encode().unwrap(), bytes);
    }
    assert!("90:100/1".parse::<Endpoint>().is_err());
    for marker in [
        "",
        "90:100",
        "90:100/1@",
        "90:100/1@bad_domain",
        "90:100/1 extra",
        "90:100/65536",
        "0:100/1",
        "90:100/1..2",
    ] {
        assert!(
            Text::parse(
                format!("\x01RESCANNED {marker}\rBody\r").as_bytes(),
                Charset::Utf8
            )
            .is_err(),
            "{marker}"
        );
    }
    assert!(Text::parse(
        b"\x01RESCANNED 90:100/1\r\x01RESCANNED 90:100/1@interop\rBody\r",
        Charset::Utf8
    )
    .is_err());
}

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

//! Disposable native Telnet acceptance. Secrets are generated per run, never printed.
use sf_core::{
    CallerConfig, CallerState, CredentialHasher, LogicalPaths, NetworkTerminalDefaults,
    PasswordHashConfig, PostLoginJourney, RuntimeDatabase, SecurityLevel, TransportAdapterConfig,
    TransportConfig,
};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

const MAIN: &[u8] = b"call minute(s) left";
struct Board {
    _temp: tempfile::TempDir,
    config: PathBuf,
    database: PathBuf,
    address: SocketAddr,
    process: Child,
    log: PathBuf,
    readiness_offset: usize,
    password: Vec<u8>,
    sysop_password: Vec<u8>,
}
impl Board {
    fn new(time_minutes: u32, registration: bool) -> Self {
        Self::with_login_timeout(time_minutes, registration, 120)
    }
    fn with_login_timeout(
        time_minutes: u32,
        registration: bool,
        login_timeout_seconds: u64,
    ) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut plan =
            sf_bbs::SetupPlan::stock_defaults("D2 Acceptance Board", "Fixture Sysop", "Sysop", 2);
        plan.config.caller = CallerConfig {
            allow_new_users: registration,
            login_timeout_seconds,
            registration_timeout_seconds: 30,
            minutes_per_call: time_minutes,
            minutes_per_day: time_minutes,
            new_caller_first_day_minutes: time_minutes,
            maximum_daily_calls: 100,
            inactivity_minutes: 1,
            post_login_journey: PostLoginJourney::None,
            password: PasswordHashConfig {
                memory_kib: 8,
                iterations: 1,
                parallelism: 1,
            },
            ..plan.config.caller
        };
        plan.config.caller.profile.require_names = true;
        plan.config.caller.profile.email = sf_core::ProfileFieldPolicy::Optional;
        plan.config.transports = vec![TransportConfig {
            name: Some("d2-loopback".into()),
            enabled: true,
            adapter: TransportAdapterConfig::Telnet {
                listen: address,
                terminal: NetworkTerminalDefaults {
                    ansi: false,
                    cp437: false,
                    width: 100,
                    height: 100,
                },
            },
        }];
        let password = format!(
            "{:032x}{:032x}",
            rand::random::<u128>(),
            rand::random::<u128>()
        )
        .into_bytes();
        let sysop_password = format!("{:032x}", rand::random::<u128>()).into_bytes();
        sf_bbs::setup_board(&root, &plan, &sysop_password).unwrap();
        let config = root.join(sf_bbs::BOARD_CONFIG_FILE);
        let paths = LogicalPaths::resolve(&root, &plan.config.validate().unwrap()).unwrap();
        let database = paths.database().to_path_buf();
        let mut db = RuntimeDatabase::open(&database).unwrap();
        let hasher = CredentialHasher::new(&plan.config.caller.password).unwrap();
        let hash = hasher.hash(&password).unwrap();
        for (login, handle, state) in [
            ("returning", "Public Handle", CallerState::Active),
            ("other", "Other Caller", CallerState::Active),
            ("disabled", "Disabled Caller", CallerState::Disabled),
        ] {
            db.create_caller_with_login_profile(
                Some(login.as_bytes()),
                handle.as_bytes(),
                &hash,
                SecurityLevel::new(10).unwrap(),
                state,
                true,
                chrono::Utc::now().timestamp(),
                Default::default(),
                &Default::default(),
            )
            .unwrap();
        }
        drop(db);
        let log = temp.path().join("daemon.log");
        let process = start(&config, &log);
        Self {
            _temp: temp,
            config,
            database,
            address,
            process,
            log,
            readiness_offset: 0,
            password,
            sysop_password,
        }
    }
    fn client(&self) -> Client {
        let end = Instant::now() + Duration::from_secs(20);
        while !fs::read(&self.log).is_ok_and(|log| {
            has(
                &log[self.readiness_offset.min(log.len())..],
                b"transport listener ready",
            )
        }) {
            assert!(Instant::now() < end, "daemon listener did not become ready");
            thread::sleep(Duration::from_millis(20));
        }
        // EOF and durable account settlement can precede node lease release.
        // Reconnect only when the operator surface confirms a free logical node.
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut operator = sf_bbs::OperatorClient::connect(&self.config).await.unwrap();
            loop {
                if operator
                    .nodes()
                    .await
                    .unwrap()
                    .iter()
                    .any(|n| n.session_id.is_none())
                {
                    break;
                }
                assert!(
                    Instant::now() < end,
                    "logical node did not become available"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        Client::connect(self.address)
    }
    fn count(&self, sql: &str) -> i64 {
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
    fn claims(&self, expected: i64) {
        let end = Instant::now() + Duration::from_secs(10);
        while self.count("SELECT count(*) FROM caller_sessions") != expected {
            assert!(Instant::now() < end, "session ownership did not settle");
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn restart(&mut self) {
        self.process.kill().unwrap();
        self.process.wait().unwrap();
        self.readiness_offset = fs::read(&self.log).unwrap().len();
        self.process = start(&self.config, &self.log);
    }
}
fn start(config: &std::path::Path, log: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_spitfire"))
        .arg("run")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .unwrap(),
        ))
        .stderr(Stdio::from(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .unwrap(),
        ))
        .spawn()
        .unwrap()
}
impl Drop for Board {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let log = fs::read(&self.log).unwrap();
        if std::thread::panicking() {
            if let Some(path) = std::env::var_os("D2_DIAGNOSTIC_LOG") {
                fs::write(path, &log).unwrap();
            }
        }
        assert!(
            !has(&log, &self.password) && !has(&log, &self.sysop_password),
            "secret appeared in daemon log"
        );
    }
}
fn has(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|w| w == needle)
}
struct Client {
    stream: TcpStream,
    transcript: Vec<u8>,
    pending: Vec<u8>,
}
impl Client {
    fn connect(address: SocketAddr) -> Self {
        let end = Instant::now() + Duration::from_secs(20);
        let stream = loop {
            if let Ok(s) = TcpStream::connect(address) {
                break s;
            }
            assert!(Instant::now() < end, "daemon did not listen");
            thread::sleep(Duration::from_millis(30));
        };
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Self {
            stream,
            transcript: Vec::new(),
            pending: Vec::new(),
        }
    }
    fn line(&mut self, line: &[u8]) {
        let mut encoded = Vec::new();
        for b in line {
            encoded.push(*b);
            if *b == 255 {
                encoded.push(255);
            }
        }
        encoded.extend_from_slice(b"\r\n");
        self.stream.write_all(&encoded).unwrap();
    }
    #[track_caller]
    fn until(&mut self, needle: &[u8]) -> Vec<u8> {
        self.any(&[needle], Duration::from_secs(15)).0
    }
    #[track_caller]
    fn any(&mut self, needles: &[&[u8]], timeout: Duration) -> (Vec<u8>, usize) {
        let mut data = std::mem::take(&mut self.pending);
        let end = Instant::now() + timeout;
        loop {
            for (i, n) in needles.iter().enumerate() {
                if let Some(start) = data.windows(n.len()).position(|w| w == *n) {
                    self.pending = data.split_off(start + n.len());
                    return (data, i);
                }
            }
            let mut buf = [0; 4096];
            match self.stream.read(&mut buf) {
                Ok(0) => panic!(
                    "unexpected EOF before expected prompt: {}",
                    String::from_utf8_lossy(needles[0])
                ),
                Ok(n) => {
                    data.extend_from_slice(&buf[..n]);
                    self.transcript.extend_from_slice(&buf[..n]);
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(_) => panic!("unexpected transport failure"),
            }
            assert!(
                Instant::now() < end,
                "expected caller prompt did not arrive"
            );
            assert!(data.len() < 1024 * 1024, "unbounded caller output");
        }
    }
    #[track_caller]
    fn existing(&mut self, login: &[u8], password: &[u8], registration: bool) -> Vec<u8> {
        if registration {
            self.until(b"(Y/N):");
            self.line(b"N");
        }
        self.until(b"Login:");
        self.line(login);
        self.until(b"Password:");
        self.line(password);
        self.until(MAIN)
    }
    fn eof(&mut self, timeout: Duration) -> Vec<u8> {
        let end = Instant::now() + timeout;
        let mut data = std::mem::take(&mut self.pending);
        loop {
            let mut b = [0; 4096];
            match self.stream.read(&mut b) {
                Ok(0) => break,
                Ok(n) => {
                    data.extend_from_slice(&b[..n]);
                    self.transcript.extend_from_slice(&b[..n]);
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
                Err(_) => panic!("unexpected close failure"),
            }
            assert!(Instant::now() < end, "caller did not disconnect");
        }
        data
    }
    fn goodbye(mut self) {
        self.line(b"G");
        let data = self.eof(Duration::from_secs(10));
        assert!(has(&data, b"Thank you for calling"));
    }
}

#[test]
fn d2_native_existing_new_user_validation_and_bounded_failure() {
    let board = Board::new(2, true);
    let mut existing = board.client();
    existing.existing(b"RETURNING", &board.password, true);
    assert!(!has(&existing.transcript, &board.password));
    existing.goodbye();
    board.claims(0);
    let mut sysop = board.client();
    sysop.existing(b"sysop", &board.sysop_password, true);
    sysop.goodbye();
    board.claims(0);
    let mut new = board.client();
    new.until(b"(Y/N):");
    new.line(b"Y");
    new.until(b"Choose Login");
    for invalid in [
        b"".as_slice(),
        b"bad login",
        b"sysop",
        b"returning",
        &[b'x'; 33],
        &[0xfe],
        b"bad\0id",
    ] {
        new.line(invalid);
        new.until(b"Choose Login");
    }
    let login = &[b'n'; 32];
    assert_eq!(login.len(), 32);
    new.line(login);
    new.until(b"Handle:");
    for invalid in [b"".as_slice(), b"Public Handle", &[b'x'; 31]] {
        new.line(invalid);
        new.until(b"Handle:");
    }
    new.line(b"New Public Caller");
    new.until(b"Choose Password:");
    new.line(&[b'x'; 129]);
    new.until(b"Choose Password:");
    new.line(b"short");
    new.until(b"Confirm Password:");
    new.line(b"short");
    new.until(b"Choose Password:");
    new.line(&board.password);
    new.until(b"Confirm Password:");
    new.line(b"mismatch");
    new.until(b"Choose Password:");
    new.line(&board.password);
    new.until(b"Confirm Password:");
    new.line(&board.password);
    new.until(b"First Name");
    new.line(b"");
    new.until(b"First Name");
    new.line(&[0xfe]);
    new.until(b"First Name");
    new.line(b"Private");
    new.until(b"Last Name");
    new.line(b"Fixture");
    new.until(b"Email:");
    new.line(b"invalid-email");
    new.until(b"Email:");
    new.line(b"");
    let entry = new.until(MAIN);
    assert!(!has(&entry, b"Private Fixture"));
    assert!(!has(&new.transcript, &board.password));
    new.goodbye();
    board.claims(0);
    let db = RuntimeDatabase::open(&board.database).unwrap();
    let caller = db.caller_by_login_identifier(login).unwrap().unwrap();
    assert_eq!(caller.display_name, "New Public Caller");
    assert_eq!(caller.security_level.get(), 10);
    assert_eq!(
        caller.profile.identity.real_name().as_deref(),
        Some("Private Fixture")
    );
    let mut again = board.client();
    again.existing(login, &board.password, true);
    again.goodbye();
    board.claims(0);
    let successes = board.count("SELECT sum(call_count) FROM callers");
    let mut failed = board.client();
    failed.until(b"(Y/N):");
    failed.line(b"N");
    for (index, login) in [b"missing".as_slice(), b"returning", b"disabled"]
        .into_iter()
        .enumerate()
    {
        failed.until(b"Login:");
        failed.line(login);
        failed.until(b"Password:");
        failed.line(b"wrong-attempt");
        if index == 2 {
            let tail = failed.eof(Duration::from_secs(10));
            assert!(!has(&tail, b"MAIN MENU"));
        }
    }
    board.claims(0);
    assert_eq!(
        board.count("SELECT sum(call_count) FROM callers"),
        successes
    );
    let mut disabled = board.client();
    disabled.until(b"(Y/N):");
    disabled.line(b"N");
    disabled.until(b"Login:");
    disabled.line(b"disabled");
    disabled.until(b"Password:");
    disabled.line(&board.password);
    let output = disabled.eof(Duration::from_secs(10));
    assert!(!has(&output, b"MAIN MENU"));
    board.claims(0);
    assert_eq!(
        board.count("SELECT sum(call_count) FROM callers"),
        successes
    );
}

#[test]
fn d2_native_two_nodes_exactly_one_account_winner_and_restart() {
    let mut board = Board::new(2, false);
    let mut clients = [board.client(), board.client()];
    for c in &mut clients {
        c.until(b"Login:");
        c.line(b"returning");
        c.until(b"Password:");
    }
    let barrier = Arc::new(Barrier::new(2));
    let handles = clients
        .into_iter()
        .map(|mut c| {
            let barrier = Arc::clone(&barrier);
            let password = board.password.clone();
            thread::spawn(move || {
                barrier.wait();
                c.line(&password);
                let (_, result) = c.any(&[MAIN, b"already logged in"], Duration::from_secs(15));
                (c, result)
            })
        })
        .collect::<Vec<_>>();
    let mut winner = None;
    let mut denied = 0;
    for h in handles {
        let (mut c, result) = h.join().unwrap();
        if result == 0 {
            assert!(winner.is_none());
            winner = Some(c);
        } else {
            c.eof(Duration::from_secs(10));
            denied += 1;
        }
    }
    assert_eq!(denied, 1);
    board.claims(1);
    assert_eq!(
        board.count("SELECT call_count FROM callers WHERE login_identifier='returning'"),
        1
    );
    assert!(board.count("SELECT sum(reserved_seconds) FROM caller_sessions") <= 120);
    let mut other = board.client();
    other.existing(b"other", &board.password, false);
    board.claims(2);
    assert_eq!(
        board.count("SELECT count(DISTINCT node_id) FROM caller_sessions"),
        2
    );
    assert_eq!(
        board.count("SELECT count(DISTINCT caller_id) FROM caller_sessions"),
        2
    );
    drop(other);
    board.claims(1);
    assert_eq!(board.count("SELECT count(*) FROM caller_sessions s JOIN callers c USING(caller_id) WHERE c.login_identifier='returning'"), 1);
    winner.unwrap().goodbye();
    board.claims(0);
    let mut interrupted = board.client();
    interrupted.existing(b"returning", &board.password, false);
    board.claims(1);
    board.restart();
    drop(interrupted);
    let mut reconnect = board.client();
    reconnect.existing(b"returning", &board.password, false);
    board.claims(1);
    assert_eq!(board.count("SELECT count(*) FROM operational_events WHERE event_code='session.interrupted-recovered'"),1);
    reconnect.goodbye();
    board.claims(0);
}

#[test]
fn d2_native_abandonment_phase_deadlines_idle_and_call_deadline() {
    let board = Board::new(2, true);
    let original = board.count("SELECT count(*) FROM callers");
    let mut abandoned = board.client();
    abandoned.until(b"(Y/N):");
    abandoned.line(b"Y");
    abandoned.until(b"Choose Login");
    abandoned.line(b"abandoned");
    abandoned.until(b"Handle:");
    abandoned.line(b"Abandoned Caller");
    abandoned.until(b"Choose Password:");
    abandoned.stream.write_all(&board.password[..9]).unwrap();
    drop(abandoned);
    board.claims(0);
    {
        // A short login budget belongs only to this no-input timeout case.
        // Interactive registration checks use the normal login budget above.
        let login_board = Board::with_login_timeout(2, true, 10);
        let mut login = login_board.client();
        login.until(b"(Y/N):");
        let timeout = login.eof(Duration::from_secs(15));
        assert!(has(&timeout, b"Login time expired"));
        login_board.claims(0);
        assert_eq!(login_board.count("SELECT sum(call_count) FROM callers"), 0);
    }
    let mut registration = board.client();
    registration.until(b"(Y/N):");
    registration.line(b"Y");
    registration.until(b"Choose Login");
    let timeout = registration.eof(Duration::from_secs(35));
    assert!(has(&timeout, b"Registration time expired"));
    assert_eq!(board.count("SELECT count(*) FROM callers"), original);
    let mut idle = board.client();
    idle.existing(b"other", &board.password, true);
    let output = idle.eof(Duration::from_secs(70));
    assert!(has(&output, b"activity") || has(&output, b"asleep"));
    board.claims(0);
    // Model an account with one second of eligible time remaining, using only
    // disposable fixture data. Native admission and timeout run unchanged.
    let day = sf_core::board_local_day(chrono::Utc::now().timestamp(), chrono_tz::UTC).unwrap();
    rusqlite::Connection::open(&board.database).unwrap().execute(
        "UPDATE callers SET daily_usage_day=?1,daily_time_seconds=119 WHERE login_identifier='returning'",[day]).unwrap();
    let mut limited = board.client();
    limited.existing(b"returning", &board.password, true);
    let output = limited.eof(Duration::from_secs(8));
    assert!(has(&output, b"call time has expired"));
    board.claims(0);
}

#[test]
fn d2_native_registration_cancel_and_profile_eof_are_atomic() {
    let board = Board::new(2, true);
    let original = board.count("SELECT count(*) FROM callers");
    let mut cancelled = board.client();
    cancelled.until(b"(Y/N):");
    cancelled.line(b"Y");
    cancelled.until(b"Choose Login");
    cancelled.line(b"/Q");
    cancelled.eof(Duration::from_secs(10));
    for cancel_profile in [false, true] {
        let mut c = board.client();
        c.until(b"(Y/N):");
        c.line(b"Y");
        c.until(b"Choose Login");
        c.line(b"unfinished");
        c.until(b"Handle:");
        c.line(b"Unfinished Caller");
        c.until(b"Choose Password:");
        c.line(&board.password);
        c.until(b"Confirm Password:");
        c.line(&board.password);
        c.until(b"First Name");
        if cancel_profile {
            c.line(b"/Q");
            c.until(b"Login:");
        } else {
            c.line(b"Private");
            c.until(b"Last Name");
            c.stream.write_all(b"Incomplete").unwrap();
        }
        drop(c);
        board.claims(0);
        assert_eq!(board.count("SELECT count(*) FROM callers"), original);
        assert_eq!(board.count("SELECT sum(call_count) FROM callers"), 0);
    }
}

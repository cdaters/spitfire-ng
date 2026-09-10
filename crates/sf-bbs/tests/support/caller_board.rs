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

//! Shared disposable native Telnet acceptance. Secrets are generated per run, never printed.
#![allow(dead_code)] // Each milestone uses a different subset of the shared fixture.
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
    thread,
    time::{Duration, Instant},
};

pub(crate) const MAIN: &[u8] = b"call minute(s) left";
pub(crate) struct Board {
    _temp: tempfile::TempDir,
    pub(crate) config: PathBuf,
    pub(crate) database: PathBuf,
    pub(crate) address: SocketAddr,
    pub(crate) process: Child,
    pub(crate) log: PathBuf,
    readiness_offset: usize,
    pub(crate) password: Vec<u8>,
    pub(crate) sysop_password: Vec<u8>,
}
impl Board {
    pub(crate) fn new(time_minutes: u32, registration: bool) -> Self {
        Self::with_login_timeout(time_minutes, registration, 120)
    }
    pub(crate) fn with_login_timeout(
        time_minutes: u32,
        registration: bool,
        login_timeout_seconds: u64,
    ) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("board");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut plan = sf_bbs::SetupPlan::stock_defaults(
            "Caller Acceptance Board",
            "Fixture Sysop",
            "Sysop",
            2,
        );
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
            name: Some("caller-loopback".into()),
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
    pub(crate) fn client(&self) -> Client {
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
    pub(crate) fn count(&self, sql: &str) -> i64 {
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
    pub(crate) fn claims(&self, expected: i64) {
        let end = Instant::now() + Duration::from_secs(10);
        while self.count("SELECT count(*) FROM caller_sessions") != expected {
            assert!(Instant::now() < end, "session ownership did not settle");
            thread::sleep(Duration::from_millis(20));
        }
    }
    pub(crate) fn restart(&mut self) {
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
pub(crate) fn has(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|w| w == needle)
}
pub(crate) struct Client {
    pub(crate) stream: TcpStream,
    pub(crate) transcript: Vec<u8>,
    pub(crate) pending: Vec<u8>,
}
impl Client {
    pub(crate) fn connect(address: SocketAddr) -> Self {
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
    pub(crate) fn line(&mut self, line: &[u8]) {
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
    pub(crate) fn until(&mut self, needle: &[u8]) -> Vec<u8> {
        self.any(&[needle], Duration::from_secs(15)).0
    }
    #[track_caller]
    pub(crate) fn any(&mut self, needles: &[&[u8]], timeout: Duration) -> (Vec<u8>, usize) {
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
    pub(crate) fn existing(
        &mut self,
        login: &[u8],
        password: &[u8],
        registration: bool,
    ) -> Vec<u8> {
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
    pub(crate) fn eof(&mut self, timeout: Duration) -> Vec<u8> {
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
    pub(crate) fn goodbye(mut self) {
        self.line(b"G");
        let data = self.eof(Duration::from_secs(10));
        assert!(has(&data, b"Thank you for calling"));
    }
}

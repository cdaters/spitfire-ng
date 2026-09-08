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

//! Bounded full-duplex BinkP 1.0/1.1 session over an owned TCP stream.
use sf_core::ftn::{BinkpAuth, BinkpMode};
use sf_net::{
    binkp::{self, Command, Error, Frame, Offer, Phase},
    ftn::Endpoint,
};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    time::{Duration, Instant},
};

pub(super) struct Plan {
    pub local: Vec<Endpoint>,
    pub remote: Endpoint,
    pub secret: Vec<u8>,
    pub auth: BinkpAuth,
    pub domainless: bool,
    pub mode: BinkpMode,
}
pub(super) struct Outgoing {
    pub key: String,
    pub offer: Offer,
}
pub(super) trait Backend {
    fn identify(&mut self, addresses: &str) -> Result<Plan, Error>;
    fn ready(
        &mut self,
        addresses: &[Endpoint],
        capabilities: &[String],
        latency: u64,
    ) -> Result<Vec<Outgoing>, Error>;
    /// Recheck only this authenticated peer's durable file queue at a batch boundary.
    fn next_batch(&mut self, _files: usize, _bytes: u64) -> Result<Vec<Outgoing>, Error> {
        Ok(vec![])
    }
    fn load(&mut self, key: &str) -> Result<Vec<u8>, Error>;
    fn offered(&mut self, key: &str) -> Result<(), Error>;
    fn accepted(&mut self, key: &str) -> Result<(), Error>;
    fn receive(&mut self, bytes: &[u8]) -> Result<(), Error>;
    fn accepts_file(&self, name: &str) -> bool {
        name.to_ascii_lowercase().ends_with(".pkt")
    }
    fn receive_file(&mut self, offer: &Offer, bytes: &[u8]) -> Result<(), Error> {
        let _ = offer;
        self.receive(bytes)
    }
    fn cancelled(&self) -> bool;
}
pub(super) struct Limits {
    pub handshake: Duration,
    pub idle: Duration,
    pub session: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(30),
            idle: Duration::from_secs(60),
            session: Duration::from_secs(600),
        }
    }
}
#[derive(Default)]
pub(super) struct Summary {
    pub sent: u32,
    pub received: u32,
    pub skipped: u32,
}
struct Wire {
    socket: TcpStream,
    input: Vec<u8>,
    output: VecDeque<Vec<u8>>,
    written: usize,
    last: Instant,
}
impl Wire {
    fn new(socket: TcpStream) -> Result<Self, Error> {
        socket
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        socket.set_nodelay(true).map_err(|_| Error::Unavailable)?;
        Ok(Self {
            socket,
            input: vec![],
            output: VecDeque::new(),
            written: 0,
            last: Instant::now(),
        })
    }
    fn send(&mut self, command: Command, args: &str) -> Result<(), Error> {
        self.push(Frame::command(command, args)?)
    }
    fn push(&mut self, frame: Frame) -> Result<(), Error> {
        if self.output.len() >= 64 {
            return Err(Error::Limit);
        }
        self.output.push_back(frame.encode()?);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        if let Some(bytes) = self.output.front() {
            match self.socket.write(&bytes[self.written..]) {
                Ok(0) => return Err(Error::Interrupted),
                Ok(n) => {
                    self.last = Instant::now();
                    self.written += n;
                    if self.written == bytes.len() {
                        trace_frame("send", &Frame::decode(bytes)?);
                        self.output.pop_front();
                        self.written = 0;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err(Error::Interrupted),
            }
        }
        Ok(())
    }
    fn frame(&mut self) -> Result<Option<Frame>, Error> {
        // Read only the missing header/payload bytes; hostile data cannot grow a backlog.
        let needed = if self.input.len() < 2 {
            2
        } else {
            let size = usize::from(u16::from_be_bytes([self.input[0], self.input[1]]) & 0x7fff);
            if self.input[0] & 0x80 != 0 && size > binkp::MAX_COMMAND {
                return Err(Error::Limit);
            }
            size + 2
        };
        if self.input.len() < needed {
            let mut buffer = [0u8; 8192];
            let count = (needed - self.input.len()).min(buffer.len());
            match self.socket.read(&mut buffer[..count]) {
                Ok(0) => return Err(Error::Interrupted),
                Ok(n) => {
                    self.input.extend_from_slice(&buffer[..n]);
                    self.last = Instant::now();
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(None)
                }
                Err(_) => return Err(Error::Interrupted),
            }
        }
        if self.input.len() < 2 {
            return Ok(None);
        }
        let size = usize::from(u16::from_be_bytes([self.input[0], self.input[1]]) & 0x7fff) + 2;
        if self.input.len() != size {
            return Ok(None);
        }
        let frame = Frame::decode(&self.input)?;
        self.input.clear();
        trace_frame("receive", &frame);
        Ok(Some(frame))
    }
    fn abort(&mut self, error: Error) {
        // Never echo peer text, secrets or artifact data. Drain a partially written frame
        // before the finite error command, bounded by a short graceful-finish deadline.
        if self.written == 0 {
            self.output.clear();
        } else {
            self.output.truncate(1);
        }
        let _ = self.send(
            if error == Error::Busy {
                Command::Bsy
            } else {
                Command::Err
            },
            &error.to_string(),
        );
        let until = Instant::now() + Duration::from_millis(100);
        while !self.output.is_empty() && Instant::now() < until {
            if self.flush().is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
/// Opt-in protocol metadata only: never format raw arguments, authentication
/// material, human NUL text, or payload bytes, including on a parse failure.
fn trace_frame(direction: &str, frame: &Frame) {
    match frame {
        Frame::Command(id, args) => {
            let command = Command::from_id(*id);
            if matches!(
                command,
                Some(Command::File | Command::Got | Command::Get | Command::Skip)
            ) {
                if let Ok(args) = text(args) {
                    if let Ok((offer, _)) = file_command(
                        args,
                        matches!(command, Some(Command::File | Command::Get)),
                        true,
                        true,
                    ) {
                        tracing::debug!(target: "sfng_binkp_metadata", direction, ?command, name = %offer.name, size = offer.size, time = offer.time);
                        return;
                    }
                }
            }
            tracing::debug!(target: "sfng_binkp_metadata", direction, ?command);
        }
        Frame::Data(bytes) => {
            tracing::debug!(target: "sfng_binkp_metadata", direction, event = "data", size = bytes.len())
        }
        Frame::Empty => {
            tracing::debug!(target: "sfng_binkp_metadata", direction, event = "empty-frame")
        }
    }
}
fn text(args: &[u8]) -> Result<&str, Error> {
    if args.iter().any(|b| !(32..=126).contains(b)) {
        return Err(Error::Malformed);
    }
    std::str::from_utf8(args).map_err(|_| Error::Malformed)
}
fn advertise(wire: &mut Wire, plan: &Plan) -> Result<(), Error> {
    if plan.local.is_empty() || plan.local.len() > 32 {
        return Err(Error::Address);
    }
    wire.send(Command::Nul, "VER SPITFIRE-NG/0.1 binkp/1.1")?;
    wire.send(
        Command::Adr,
        &plan
            .local
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
    )
}
fn options(args: &str, caps: &mut Vec<String>, cram: &mut Option<Vec<u8>>) -> Result<(), Error> {
    if let Some(list) = args.strip_prefix("OPT ") {
        for option in list.split_ascii_whitespace() {
            if option.starts_with("CRAM-") {
                if cram.is_some() {
                    return Err(Error::Authentication);
                }
                *cram = Some(binkp::challenge(option)?);
                if !caps.iter().any(|c| c == "CRAM-MD5") {
                    caps.push("CRAM-MD5".into());
                }
            } else if matches!(
                option,
                "NR" | "ND" | "NDA" | "MB" | "CRYPT" | "GZ" | "BZ2" | "CRC"
            ) && !caps.iter().any(|c| c == option)
            {
                caps.push(option.into());
            }
            if caps.len() > 32 {
                return Err(Error::Limit);
            }
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Protocol {
    #[default]
    OneBatch,
    MultipleBatches,
}
fn version(args: &str) -> Option<Protocol> {
    let rest = args.strip_prefix("VER ")?;
    Some(
        if rest
            .split_ascii_whitespace()
            .last()?
            .eq_ignore_ascii_case("binkp/1.1")
        {
            Protocol::MultipleBatches
        } else {
            Protocol::OneBatch
        },
    )
}
/// Empty-batch evidence is separate from transfer acknowledgements. It never
/// grants file custody or changes durable request/response authorization.
#[derive(Default)]
struct Batches {
    protocol: Protocol,
    sent: u8,
    received: u8,
    reply: bool,
    sent_total: u16,
    received_total: u16,
}
impl Batches {
    fn activity(&mut self) {
        if self.protocol == Protocol::MultipleBatches {
            self.received = 0;
            self.reply = false;
        }
    }
    fn outgoing(&mut self) -> Result<(), Error> {
        if self.protocol == Protocol::OneBatch && self.sent != 0 {
            return Err(Error::Malformed);
        }
        self.sent = 0;
        Ok(())
    }
    fn confirmation(&mut self) {
        // Peer ACKs may straddle its send batches. Our confirmation requires
        // fresh local empty-batch evidence, without erasing the peer's EOBs.
        if self.protocol == Protocol::MultipleBatches {
            self.sent = 0;
        }
    }
    fn receive_eob(&mut self) -> Result<(), Error> {
        self.received_total += 1;
        if self.received_total > 128 {
            return Err(Error::Limit);
        }
        self.received = self.received.saturating_add(1).min(2);
        self.reply = true;
        Ok(())
    }
    fn send_eob(&mut self) -> Result<(), Error> {
        self.sent_total += 1;
        if self.sent_total > 128 {
            return Err(Error::Limit);
        }
        self.sent = self.sent.saturating_add(1).min(2);
        self.reply = false;
        Ok(())
    }
    fn needs_eob(&self) -> bool {
        self.sent == 0 || (self.protocol == Protocol::MultipleBatches && self.reply)
    }
    fn complete(&self) -> bool {
        let count = if self.protocol == Protocol::MultipleBatches {
            2
        } else {
            1
        };
        self.sent >= count && self.received >= count
    }
    fn accepts_file(&self) -> bool {
        self.protocol == Protocol::MultipleBatches || self.received == 0
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileOffset {
    Data,
    Request,
}
/// FTS-1028 NR is normalized only at the session-command boundary. The ordinary
/// Offer parser still rejects negative offsets and malformed identity metadata.
fn file_command(
    args: &str,
    offset: bool,
    nr: bool,
    extended: bool,
) -> Result<(Offer, FileOffset), Error> {
    let mut fields: Vec<_> = args.split_ascii_whitespace().collect();
    let count = if offset { 4 } else { 3 };
    if extended && fields.len() > count {
        fields.truncate(count);
    }
    let request = nr && offset && fields.get(3) == Some(&"-1");
    if request {
        fields[3] = "0";
    }
    Ok((
        Offer::parse(&fields.join(" "), offset)?,
        if request {
            FileOffset::Request
        } else {
            FileOffset::Data
        },
    ))
}
struct Sending {
    item: Outgoing,
    bytes: Vec<u8>,
    position: usize,
    waiting_get: bool,
}
struct Receiving {
    offer: Offer,
    bytes: Vec<u8>,
}
pub(super) fn run(
    socket: TcpStream,
    initial: Option<Plan>,
    backend: &mut dyn Backend,
    limits: Limits,
) -> Result<Summary, Error> {
    let mut wire = Wire::new(socket)?;
    let result = exchange(&mut wire, initial, backend, limits);
    tracing::debug!(target: "sfng_binkp_metadata", event = "session-close", error = ?result.as_ref().err());
    if let Err(error) = result {
        wire.abort(error);
    } else {
        let _ = wire.socket.shutdown(Shutdown::Both);
    }
    result
}
fn exchange(
    wire: &mut Wire,
    mut plan: Option<Plan>,
    backend: &mut dyn Backend,
    limits: Limits,
) -> Result<Summary, Error> {
    let started = Instant::now();
    let caller = plan.is_some();
    let mut phase = Phase::Greeting;
    let fresh = rand::random::<[u8; 32]>();
    if let Some(p) = &plan {
        advertise(wire, p)?;
    } else {
        wire.send(
            Command::Nul,
            &format!("OPT CRAM-MD5-{}", binkp::hex(&fresh)),
        )?;
    }
    let mut remote: Option<Vec<Endpoint>> = None;
    let mut challenge = None;
    let mut caps = vec![];
    let mut authenticated = false;
    let mut password_sent = false;
    let mut greeting_seen = false;
    let mut command_count = 0u32;
    let mut pending = VecDeque::new();
    let mut sending: Option<Sending> = None;
    let mut receiving: Option<Receiving> = None;
    let mut discard = false;
    let mut requested: Option<Offer> = None;
    let mut offered_count = 0usize;
    let mut received_bytes = 0u64;
    let mut transmitted_bytes = 0u64;
    let mut batches = Batches::default();
    let mut advertised_version = None;
    let mut outbound_count = 0usize;
    let mut outbound_bytes = 0u64;
    let mut recheck = false;
    let mut summary = Summary::default();
    loop {
        if backend.cancelled() {
            return Err(Error::Cancelled);
        }
        if started.elapsed() > limits.session
            || wire.last.elapsed() > limits.idle
            || (!authenticated && started.elapsed() > limits.handshake)
        {
            return Err(Error::Timeout);
        }
        wire.flush()?;
        if authenticated
            && batches.complete()
            && (!recheck || batches.protocol == Protocol::OneBatch)
            && pending.is_empty()
            && sending.is_none()
            && receiving.is_none()
            && wire.output.is_empty()
        {
            return Ok(summary);
        }
        if let Some(frame) = wire.frame()? {
            match frame {
                Frame::Empty => {
                    command_count += 1;
                }
                Frame::Data(bytes) => {
                    if !authenticated || !batches.accepts_file() || requested.is_some() {
                        return Err(Error::Malformed);
                    }
                    received_bytes = received_bytes
                        .checked_add(bytes.len() as u64)
                        .ok_or(Error::Limit)?;
                    if received_bytes > binkp::MAX_SESSION_BYTES {
                        return Err(Error::Limit);
                    }
                    if let Some(r) = &mut receiving {
                        if r.bytes.len() + bytes.len() > r.offer.size as usize {
                            return Err(Error::Malformed);
                        }
                        r.bytes.extend(bytes);
                        if r.bytes.len() == r.offer.size as usize {
                            let r = receiving.take().ok_or(Error::Malformed)?;
                            backend.receive_file(&r.offer, &r.bytes)?;
                            recheck = true;
                            batches.confirmation();
                            wire.send(Command::Got, &r.offer.arguments(false))?;
                            summary.received += 1;
                        }
                    } else if !discard {
                        return Err(Error::Malformed);
                    }
                }
                Frame::Command(id, args) => {
                    command_count += 1;
                    if command_count > 4096 {
                        return Err(Error::Limit);
                    }
                    let Some(command) = Command::from_id(id) else {
                        continue;
                    };
                    if command == Command::Err {
                        return Err(if phase == Phase::Authenticating {
                            Error::Authentication
                        } else {
                            Error::Refused
                        });
                    }
                    if command == Command::Bsy {
                        return Err(Error::Busy);
                    }
                    // NUL may contain legacy high bytes. Only ASCII OPT tokens are interpreted.
                    if command == Command::Nul {
                        if let Ok(args) = text(&args) {
                            if !authenticated && !password_sent {
                                if let Some(v) = version(args) {
                                    if advertised_version.is_some_and(|old| old != v) {
                                        return Err(Error::Malformed);
                                    }
                                    advertised_version = Some(v);
                                    batches.protocol = v;
                                }
                            }
                            options(args, &mut caps, &mut challenge)?;
                        }
                        if !greeting_seen {
                            greeting_seen = true;
                            if caller
                                && challenge.is_none()
                                && plan
                                    .as_ref()
                                    .is_some_and(|p| p.auth == BinkpAuth::RequireCram)
                            {
                                return Err(Error::Authentication);
                            }
                        }
                        continue;
                    }
                    let args = text(&args)?;
                    if !authenticated {
                        match command {
                            Command::Adr => {
                                if remote.is_some() {
                                    return Err(Error::Address);
                                }
                                if !caller {
                                    let selected = backend.identify(args)?;
                                    advertise(wire, &selected)?;
                                    plan = Some(selected);
                                }
                                let p = plan.as_ref().ok_or(Error::Address)?;
                                let addresses = binkp::addresses(
                                    args,
                                    p.domainless.then_some(&p.remote.domain),
                                )?;
                                if !addresses.contains(&p.remote) {
                                    return Err(Error::Address);
                                }
                                remote = Some(addresses);
                                phase = Phase::Authenticating;
                                if caller {
                                    let pwd = if let Some(c) = &challenge {
                                        binkp::cram(&p.secret, c)
                                    } else if p.auth == BinkpAuth::AllowPlain {
                                        String::from_utf8(p.secret.clone())
                                            .map_err(|_| Error::Authentication)?
                                    } else {
                                        return Err(Error::Authentication);
                                    };
                                    wire.send(Command::Pwd, &pwd)?;
                                    password_sent = true;
                                }
                            }
                            Command::Pwd if !caller && remote.is_some() => {
                                let p = plan.as_ref().ok_or(Error::Authentication)?;
                                if args.starts_with("CRAM-") {
                                    binkp::verify_cram(
                                        &p.secret,
                                        &fresh,
                                        args.split_ascii_whitespace()
                                            .next()
                                            .ok_or(Error::Authentication)?,
                                    )?;
                                } else if p.auth == BinkpAuth::AllowPlain {
                                    // Equal-key HMAC comparison avoids a plain early-exit secret comparison.
                                    binkp::verify_cram(
                                        &p.secret,
                                        &fresh,
                                        &binkp::cram(args.as_bytes(), &fresh),
                                    )?;
                                } else {
                                    return Err(Error::Authentication);
                                }
                                wire.send(Command::Ok, "secure")?;
                                authenticated = true;
                            }
                            Command::Ok if caller && password_sent && remote.is_some() => {
                                if args == "non-secure" {
                                    return Err(Error::Authentication);
                                }
                                authenticated = true;
                            }
                            _ => return Err(Error::Malformed),
                        }
                        if authenticated {
                            phase = Phase::Ready;
                            let work = backend.ready(
                                remote.as_deref().ok_or(Error::Address)?,
                                &caps,
                                started.elapsed().as_millis() as u64,
                            )?;
                            if work.len() > binkp::MAX_FILES {
                                return Err(Error::Limit);
                            }
                            if plan.as_ref().is_some_and(|p| p.mode == BinkpMode::Test)
                                && !work.is_empty()
                            {
                                return Err(Error::Malformed);
                            }
                            outbound_count = work.len();
                            outbound_bytes = work.iter().map(|w| w.offer.size).sum();
                            if outbound_bytes > binkp::MAX_SESSION_BYTES {
                                return Err(Error::Limit);
                            }
                            pending = work.into();
                        }
                    } else {
                        match command {
                            Command::File => {
                                if !batches.accepts_file() {
                                    return Err(Error::Malformed);
                                }
                                offered_count += 1;
                                if offered_count > binkp::MAX_FILES {
                                    return Err(Error::Limit);
                                }
                                batches.activity();
                                let multiple = batches.protocol == Protocol::MultipleBatches;
                                let (offer, offset) = file_command(
                                    args,
                                    true,
                                    multiple || caps.iter().any(|c| c == "NR"),
                                    multiple,
                                )?;
                                if requested.as_ref().is_some_and(|old| !old.same_file(&offer)) {
                                    return Err(Error::Malformed);
                                }
                                requested = None;
                                // An interrupted/replaced partial stays private and is discarded.
                                receiving = None;
                                discard = false;
                                if plan.as_ref().is_some_and(|p| p.mode == BinkpMode::Test)
                                    || !backend.accepts_file(&offer.name)
                                {
                                    batches.confirmation();
                                    wire.send(Command::Skip, &offer.arguments(false))?;
                                    discard = true;
                                    summary.skipped += 1;
                                } else if offset == FileOffset::Request || offer.offset != 0 {
                                    let restart = Offer { offset: 0, ..offer };
                                    batches.confirmation();
                                    wire.send(Command::Get, &restart.arguments(true))?;
                                    requested = Some(restart);
                                    discard = true;
                                } else {
                                    if received_bytes + offer.size > binkp::MAX_SESSION_BYTES {
                                        return Err(Error::Limit);
                                    }
                                    receiving = Some(Receiving {
                                        offer,
                                        bytes: Vec::new(),
                                    });
                                }
                            }
                            Command::Got | Command::Skip | Command::Get => {
                                let (offer, _) = file_command(
                                    args,
                                    command == Command::Get,
                                    false,
                                    batches.protocol == Protocol::MultipleBatches,
                                )?;
                                let s = sending.as_mut().ok_or(Error::Malformed)?;
                                if !s.item.offer.same_file(&offer) {
                                    return Err(Error::Malformed);
                                }
                                if command == Command::Get {
                                    s.position = offer.offset as usize;
                                    s.waiting_get = false;
                                    wire.send(Command::File, &offer.arguments(true))?;
                                } else {
                                    if command == Command::Got {
                                        backend.accepted(&s.item.key)?;
                                        summary.sent += 1;
                                    } else {
                                        summary.skipped += 1;
                                    }
                                    sending = None;
                                }
                            }
                            Command::Eob => {
                                if receiving.is_some() || requested.is_some() {
                                    return Err(Error::Interrupted);
                                }
                                discard = false;
                                batches.receive_eob()?;
                                recheck = true;
                            }
                            _ => return Err(Error::Malformed),
                        }
                    }
                }
            }
        }
        if command_count > 4096 {
            return Err(Error::Limit);
        }
        if authenticated && wire.output.is_empty() {
            phase = Phase::Exchanging;
            if sending.is_none() {
                if pending.is_empty()
                    && batches.protocol == Protocol::MultipleBatches
                    && (recheck || batches.needs_eob())
                {
                    let work = backend.next_batch(
                        binkp::MAX_FILES.saturating_sub(outbound_count),
                        binkp::MAX_SESSION_BYTES.saturating_sub(outbound_bytes),
                    )?;
                    outbound_count += work.len();
                    outbound_bytes = outbound_bytes
                        .checked_add(work.iter().map(|w| w.offer.size).sum::<u64>())
                        .ok_or(Error::Limit)?;
                    if outbound_count > binkp::MAX_FILES
                        || outbound_bytes > binkp::MAX_SESSION_BYTES
                    {
                        return Err(Error::Limit);
                    }
                    pending.extend(work);
                    recheck = false;
                }
                // The boundary recheck must precede completion, but must not
                // emit an extra EOB after both empty batches are already proven.
                // That extra write races a peer's legitimate graceful close.
                if pending.is_empty()
                    && batches.complete()
                    && receiving.is_none()
                    && requested.is_none()
                {
                    return Ok(summary);
                }
                if let Some(item) = pending.pop_front() {
                    let bytes = backend.load(&item.key)?;
                    if bytes.len() as u64 != item.offer.size || item.offer.size > binkp::MAX_FILE {
                        return Err(Error::Custody);
                    }
                    backend.offered(&item.key)?;
                    batches.outgoing()?;
                    let waiting_get = caps.iter().any(|c| c == "NR");
                    let arguments = if waiting_get {
                        format!("{} -1", item.offer.arguments(false))
                    } else {
                        item.offer.arguments(true)
                    };
                    wire.send(Command::File, &arguments)?;
                    sending = Some(Sending {
                        item,
                        bytes,
                        position: 0,
                        waiting_get,
                    });
                } else if batches.needs_eob() {
                    batches.send_eob()?;
                    wire.send(Command::Eob, "")?;
                    phase = Phase::Finishing;
                }
            } else if let Some(s) = &mut sending {
                if !s.waiting_get && s.position < s.bytes.len() {
                    let end = (s.position + 8192).min(s.bytes.len());
                    transmitted_bytes += (end - s.position) as u64;
                    if transmitted_bytes > binkp::MAX_SESSION_BYTES {
                        return Err(Error::Limit);
                    }
                    wire.push(Frame::Data(s.bytes[s.position..end].to_vec()))?;
                    s.position = end;
                }
            }
        }
        if authenticated
            && batches.complete()
            && (!recheck || batches.protocol == Protocol::OneBatch)
            && pending.is_empty()
            && sending.is_none()
            && receiving.is_none()
            && wire.output.is_empty()
        {
            return Ok(summary);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    struct Peer {
        point: bool,
        files: bool,
        secret: Vec<u8>,
        send: Vec<Vec<u8>>,
        received: Vec<Vec<u8>>,
        accepted: usize,
        test: bool,
    }
    impl Peer {
        fn new(point: bool) -> Self {
            Self {
                point,
                files: false,
                secret: b"isolated-binkp-test".to_vec(),
                send: vec![vec![42; 20000], vec![24; 100]],
                received: vec![],
                accepted: 0,
                test: false,
            }
        }
        fn plan(&self) -> Plan {
            let node: Endpoint = "10:100/1@synthetic".parse().unwrap();
            let point: Endpoint = "10:100/1.3@synthetic".parse().unwrap();
            Plan {
                local: vec![if self.point {
                    point.clone()
                } else {
                    node.clone()
                }],
                remote: if self.point { node } else { point },
                secret: self.secret.clone(),
                auth: BinkpAuth::RequireCram,
                domainless: false,
                mode: if self.test {
                    BinkpMode::Test
                } else {
                    BinkpMode::Poll
                },
            }
        }
    }
    impl Backend for Peer {
        fn accepts_file(&self, name: &str) -> bool {
            name.ends_with(".pkt") || (self.files && sf_net::tic::filename(name).is_ok())
        }
        fn identify(&mut self, _: &str) -> Result<Plan, Error> {
            Ok(self.plan())
        }
        fn ready(
            &mut self,
            addresses: &[Endpoint],
            _: &[String],
            _: u64,
        ) -> Result<Vec<Outgoing>, Error> {
            assert!(addresses.contains(&self.plan().remote));
            if self.test {
                return Ok(vec![]);
            }
            Ok(self
                .send
                .iter()
                .enumerate()
                .map(|(i, b)| Outgoing {
                    key: i.to_string(),
                    offer: Offer {
                        name: format!("{i}.{}", if self.files { "ZIP" } else { "pkt" }),
                        size: b.len() as u64,
                        time: 100,
                        offset: 0,
                    },
                })
                .collect())
        }
        fn load(&mut self, key: &str) -> Result<Vec<u8>, Error> {
            Ok(self.send[key.parse::<usize>().unwrap()].clone())
        }
        fn offered(&mut self, _: &str) -> Result<(), Error> {
            Ok(())
        }
        fn accepted(&mut self, _: &str) -> Result<(), Error> {
            self.accepted += 1;
            Ok(())
        }
        fn receive(&mut self, b: &[u8]) -> Result<(), Error> {
            self.received.push(b.to_vec());
            Ok(())
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    fn limits() -> Limits {
        Limits {
            handshake: Duration::from_secs(30),
            idle: Duration::from_secs(30),
            session: Duration::from_secs(60),
        }
    }
    fn pair(
        mut caller: Peer,
        mut answerer: Peer,
    ) -> (Peer, Peer, Result<Summary, Error>, Result<Summary, Error>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, start) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(65);
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break Some(socket),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break None,
                }
            };
            // An accepted localhost socket can precede the caller's completed
            // connect by many seconds on the acceptance host. Start protocol
            // clocks only after both fixture endpoints are ready; production
            // handshake/session limits and all wire assertions remain unchanged.
            let r = if let Some(socket) = socket {
                if start.recv_timeout(Duration::from_secs(65)).is_ok() {
                    run(socket, None, &mut answerer, limits())
                } else {
                    Err(Error::Unavailable)
                }
            } else {
                Err(Error::Unavailable)
            };
            (answerer, r)
        });
        let first = match TcpStream::connect_timeout(&address, Duration::from_secs(60)) {
            Ok(socket) => {
                let _ = ready.send(());
                let plan = caller.plan();
                run(socket, Some(plan), &mut caller, limits())
            }
            Err(_) => {
                drop(ready);
                Err(Error::Unavailable)
            }
        };
        let (answerer, second) = handle.join().unwrap();
        (caller, answerer, first, second)
    }

    #[test]
    fn actual_node_point_batch_both_directions() {
        let (c, a, x, y) = pair(Peer::new(false), Peer::new(true));
        assert!(x.is_ok(), "caller={:?}, answerer={:?}", x.err(), y.err());
        assert!(y.is_ok(), "{:?}", y.err());
        assert_eq!(c.received, a.send);
        assert_eq!(a.received, c.send);
        assert_eq!(c.accepted, 2);
        assert_eq!(a.accepted, 2);
    }
    #[test]
    fn wrong_password_and_wrong_point_fail_before_custody() {
        let c = Peer::new(false);
        let mut a = Peer::new(true);
        a.secret = b"other-isolated-secret".to_vec();
        let (c, a, x, y) = pair(c, a);
        assert!(x.is_err());
        assert!(y.is_err());
        assert!(c.received.is_empty() && a.received.is_empty());
        assert_eq!(c.accepted + a.accepted, 0);
        let (c, a, x, y) = pair(Peer::new(false), Peer::new(false));
        assert!(x.is_err());
        assert!(y.is_err());
        assert!(c.received.is_empty() && a.received.is_empty());
    }
    #[test]
    fn test_link_refuses_eager_peer_mail() {
        let mut c = Peer::new(false);
        c.test = true;
        let (c, a, x, y) = pair(c, Peer::new(true));
        assert!(x.is_ok(), "caller={:?}, answerer={:?}", x.err(), y.err());
        assert!(y.is_ok());
        assert!(c.received.is_empty() && a.received.is_empty());
        assert_eq!(c.accepted + a.accepted, 0);
    }
    fn raw_read(socket: &mut TcpStream) -> Frame {
        let mut header = [0; 2];
        socket.read_exact(&mut header).unwrap();
        let size = usize::from(u16::from_be_bytes(header) & 0x7fff);
        let mut bytes = header.to_vec();
        bytes.resize(size + 2, 0);
        socket.read_exact(&mut bytes[2..]).unwrap();
        Frame::decode(&bytes).unwrap()
    }
    fn raw_send(socket: &mut TcpStream, command: Command, args: &str) {
        socket
            .write_all(
                &Frame::Command(command as u8, args.as_bytes().to_vec())
                    .encode()
                    .unwrap(),
            )
            .unwrap();
    }
    fn scripted(peer: Peer, script: impl FnOnce(&mut TcpStream)) -> (Peer, Result<Summary, Error>) {
        scripted_with(peer, None, limits(), script)
    }
    fn scripted_with(
        peer: Peer,
        protocol: Option<&str>,
        limits: Limits,
        script: impl FnOnce(&mut TcpStream),
    ) -> (Peer, Result<Summary, Error>) {
        scripted_options(peer, protocol, None, limits, script)
    }
    fn scripted_options(
        mut peer: Peer,
        protocol: Option<&str>,
        opts: Option<&str>,
        limits: Limits,
        script: impl FnOnce(&mut TcpStream),
    ) -> (Peer, Result<Summary, Error>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let result = run(socket, None, &mut peer, limits);
            (peer, result)
        });
        let mut socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let Frame::Command(_, greeting) = raw_read(&mut socket) else {
            panic!("challenge")
        };
        let mut caps = vec![];
        let mut challenge = None;
        options(
            std::str::from_utf8(&greeting).unwrap(),
            &mut caps,
            &mut challenge,
        )
        .unwrap();
        if let Some(protocol) = protocol {
            raw_send(
                &mut socket,
                Command::Nul,
                &format!("VER scripted {protocol}"),
            );
        }
        if let Some(opts) = opts {
            raw_send(&mut socket, Command::Nul, opts);
        }
        raw_send(&mut socket, Command::Adr, "10:100/1@synthetic");
        loop {
            if matches!(raw_read(&mut socket), Frame::Command(id, _) if id==Command::Adr as u8) {
                break;
            }
        }
        raw_send(
            &mut socket,
            Command::Pwd,
            &format!(
                "{} optional",
                binkp::cram(b"isolated-binkp-test", &challenge.unwrap())
            ),
        );
        loop {
            if matches!(raw_read(&mut socket), Frame::Command(id, _) if id==Command::Ok as u8) {
                break;
            }
        }
        script(&mut socket);
        let _ = socket.shutdown(Shutdown::Both);
        drop(socket);
        worker.join().unwrap()
    }
    #[test]
    fn interrupted_inbound_never_tosses_and_retry_can_complete() {
        let mut peer = Peer::new(true);
        peer.send.clear();
        let (peer, result) = scripted(peer, |socket| {
            raw_send(socket, Command::File, "incoming.pkt 100 100 0");
            socket
                .write_all(&Frame::Data(vec![7; 99]).encode().unwrap())
                .unwrap();
        });
        assert!(result.is_err());
        assert!(peer.received.is_empty());
        let mut sender = Peer::new(false);
        sender.send = vec![vec![7; 100]];
        let (caller, answerer, a, b) = pair(sender, peer);
        assert!(
            a.is_ok() && b.is_ok(),
            "caller={:?} answerer={:?}",
            a.err(),
            b.err()
        );
        assert_eq!(caller.accepted, 1);
        assert_eq!(answerer.received, vec![vec![7; 100]]);
    }
    #[test]
    fn interrupted_file_payload_never_imports_and_retry_is_accepted_once() {
        let mut peer = Peer::new(true);
        peer.send.clear();
        peer.files = true;
        let (peer, result) = scripted(peer, |socket| {
            raw_send(socket, Command::File, "BROKEN.ZIP 100 100 0");
            socket
                .write_all(&Frame::Data(vec![7; 99]).encode().unwrap())
                .unwrap();
        });
        assert!(result.is_err());
        assert!(peer.received.is_empty());
        let mut sender = Peer::new(false);
        sender.files = true;
        sender.send = vec![vec![7; 100]];
        let (caller, answerer, a, b) = pair(sender, peer);
        assert!(
            a.is_ok() && b.is_ok(),
            "caller={:?} answerer={:?}",
            a.err(),
            b.err()
        );
        assert_eq!(caller.accepted, 1);
        assert_eq!(answerer.received, vec![vec![7; 100]]);
    }
    #[test]
    fn complete_socket_write_without_got_never_accepts_outbound() {
        let (peer, result) = scripted(Peer::new(true), |socket| {
            let mut remaining = 0;
            loop {
                match raw_read(socket) {
                    Frame::Command(id, args) if id == Command::File as u8 => {
                        remaining = Offer::parse(std::str::from_utf8(&args).unwrap(), true)
                            .unwrap()
                            .size;
                    }
                    Frame::Data(bytes) => {
                        remaining -= bytes.len() as u64;
                        if remaining == 0 {
                            break;
                        }
                    }
                    _ => (),
                }
            }
        });
        assert!(result.is_err());
        assert_eq!(peer.accepted, 0);
        let (caller, answerer, a, b) = pair(Peer::new(false), peer);
        assert!(
            a.is_ok() && b.is_ok(),
            "caller={:?} answerer={:?}",
            a.err(),
            b.err()
        );
        assert_eq!(answerer.accepted, 2);
        assert_eq!(caller.received.len(), 2);
    }
    #[test]
    fn authenticated_path_and_resource_offers_fail_closed() {
        for offer in ["../escape.pkt 10 100 0", "oversize.pkt 16777217 100 0"] {
            let mut peer = Peer::new(true);
            peer.send.clear();
            let (peer, result) = scripted(peer, |socket| {
                raw_send(socket, Command::File, offer);
                loop {
                    if matches!(raw_read(socket), Frame::Command(id, _) if id==Command::Err as u8) {
                        break;
                    }
                }
            });
            assert!(result.is_err());
            assert!(peer.received.is_empty());
            assert_eq!(peer.accepted, 0);
        }
    }
    #[test]
    fn stalled_handshake_is_bounded_and_unknown_options_are_not_diagnostics() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (answer, _) = listener.accept().unwrap();
        let mut peer = Peer::new(true);
        let result = run(
            answer,
            None,
            &mut peer,
            Limits {
                handshake: Duration::from_millis(100),
                idle: Duration::from_secs(1),
                session: Duration::from_secs(1),
            },
        );
        assert_eq!(result.err(), Some(Error::Timeout));
        drop(socket);
        let mut caps = vec![];
        let mut challenge = None;
        options("OPT NR SECRET-REFLECTION MB", &mut caps, &mut challenge).unwrap();
        assert_eq!(caps, vec!["NR", "MB"]);
    }
    mod batches {
        include!("batch_tests.rs");
    }
}

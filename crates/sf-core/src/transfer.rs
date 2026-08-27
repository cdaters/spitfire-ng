//! Stock SPITFIRE file-transfer wire protocols.
//!
//! The protocol engines own a terminal's application-byte stream only for the
//! duration of a transfer. File authorization, path confinement, staging, and
//! catalog mutation remain in the file service.

use std::time::Duration;

use thiserror::Error;
use zmodem2::{Action, Event, FileInfo, Position, Receiver, Sender};

use crate::{normalize_filename, Terminal, TerminalError, MAX_FILE_NAME_BYTES};

const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;
const CRC_REQUEST: u8 = b'C';
const STREAM_REQUEST: u8 = b'G';
const CPM_EOF: u8 = 0x1a;
const TELINK_HEADER: u8 = 0x16;
const MAX_RETRIES: usize = 10;
const HANDSHAKE_RETRIES: usize = 60;
const CRC_HANDSHAKE_RETRIES: usize = 30;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TransferProtocol {
    XmodemChecksum,
    XmodemCrc,
    Xmodem1k,
    Xmodem1kG,
    YmodemBatch,
    YmodemGBatch,
    ZmodemBatch,
    Telink,
}

impl TransferProtocol {
    pub const fn stock_name(self) -> &'static str {
        match self {
            Self::XmodemChecksum => "Xmodem Checksum",
            Self::XmodemCrc => "Xmodem CRC",
            Self::Xmodem1k => "1K-Xmodem",
            Self::Xmodem1kG => "1K-Xmodem-g",
            Self::YmodemBatch => "Ymodem (Batch)",
            Self::YmodemGBatch => "Ymodem-g (Batch)",
            Self::ZmodemBatch => "Zmodem (Batch)",
            Self::Telink => "Telink",
        }
    }

    pub const fn is_batch(self) -> bool {
        matches!(
            self,
            Self::YmodemBatch | Self::YmodemGBatch | Self::ZmodemBatch
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFile {
    pub name: String,
    pub bytes: Vec<u8>,
    pub modified_unix: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivedProtocolFile {
    pub name: String,
    pub bytes: Vec<u8>,
    pub modified_unix: Option<u64>,
}

#[derive(Debug, Error)]
pub enum TransferProtocolError {
    #[error(transparent)]
    Terminal(#[from] TerminalError),
    #[error("{0} transfer timed out")]
    TimedOut(&'static str),
    #[error("{0} transfer was canceled")]
    Canceled(&'static str),
    #[error("malformed {protocol} data: {detail}")]
    Malformed {
        protocol: &'static str,
        detail: &'static str,
    },
    #[error("{0} transfer exceeded its retry limit")]
    RetryLimit(&'static str),
    #[error("{0} transfer disconnected")]
    Disconnected(&'static str),
    #[error("remote filename is not safe: {0}")]
    UnsafeFilename(String),
    #[error("incoming transfer exceeds the configured maximum of {maximum} bytes")]
    TooLarge { maximum: u64 },
    #[error("{0} supports one file per transfer")]
    SingleFileOnly(&'static str),
    #[error("ZMODEM engine failed: {0}")]
    Zmodem(String),
}

pub fn send_binary_files(
    terminal: &mut dyn Terminal,
    protocol: TransferProtocol,
    files: &[ProtocolFile],
) -> Result<(), TransferProtocolError> {
    if !protocol.is_batch() && files.len() != 1 {
        return Err(TransferProtocolError::SingleFileOnly(protocol.stock_name()));
    }
    terminal.begin_binary_mode()?;
    let result = match protocol {
        TransferProtocol::XmodemChecksum => send_xmodem(terminal, &files[0].bytes, XMode::Checksum),
        TransferProtocol::XmodemCrc => send_xmodem(terminal, &files[0].bytes, XMode::Crc),
        TransferProtocol::Xmodem1k => send_xmodem(terminal, &files[0].bytes, XMode::OneK),
        TransferProtocol::Xmodem1kG => send_xmodem(terminal, &files[0].bytes, XMode::OneKG),
        TransferProtocol::YmodemBatch => send_ymodem(terminal, files, false),
        TransferProtocol::YmodemGBatch => send_ymodem(terminal, files, true),
        TransferProtocol::ZmodemBatch => send_zmodem(terminal, files),
        TransferProtocol::Telink => send_telink(terminal, &files[0]),
    };
    finish_binary_mode(terminal, result)
}

pub fn receive_binary_files(
    terminal: &mut dyn Terminal,
    protocol: TransferProtocol,
    fallback_name: &str,
    maximum_file_bytes: u64,
    maximum_files: usize,
) -> Result<Vec<ReceivedProtocolFile>, TransferProtocolError> {
    terminal.begin_binary_mode()?;
    let result = match protocol {
        TransferProtocol::XmodemChecksum => {
            receive_xmodem(terminal, fallback_name, maximum_file_bytes, XMode::Checksum)
        }
        TransferProtocol::XmodemCrc => {
            receive_xmodem(terminal, fallback_name, maximum_file_bytes, XMode::Crc)
        }
        TransferProtocol::Xmodem1k => {
            receive_xmodem(terminal, fallback_name, maximum_file_bytes, XMode::OneK)
        }
        TransferProtocol::Xmodem1kG => {
            receive_xmodem(terminal, fallback_name, maximum_file_bytes, XMode::OneKG)
        }
        TransferProtocol::YmodemBatch => {
            receive_ymodem(terminal, maximum_file_bytes, maximum_files, false)
        }
        TransferProtocol::YmodemGBatch => {
            receive_ymodem(terminal, maximum_file_bytes, maximum_files, true)
        }
        TransferProtocol::ZmodemBatch => {
            receive_zmodem(terminal, maximum_file_bytes, maximum_files)
        }
        TransferProtocol::Telink => receive_telink(terminal, maximum_file_bytes),
    };
    finish_binary_mode(terminal, result)
}

fn finish_binary_mode<T>(
    terminal: &mut dyn Terminal,
    result: Result<T, TransferProtocolError>,
) -> Result<T, TransferProtocolError> {
    let finish = terminal.end_binary_mode();
    match (result, finish) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XMode {
    Checksum,
    Crc,
    OneK,
    OneKG,
}

impl XMode {
    const fn request(self) -> u8 {
        match self {
            Self::Checksum => NAK,
            Self::Crc | Self::OneK => CRC_REQUEST,
            Self::OneKG => STREAM_REQUEST,
        }
    }

    const fn uses_crc(self) -> bool {
        !matches!(self, Self::Checksum)
    }

    const fn streaming(self) -> bool {
        matches!(self, Self::OneKG)
    }
}

fn send_xmodem(
    terminal: &mut dyn Terminal,
    bytes: &[u8],
    mode: XMode,
) -> Result<(), TransferProtocolError> {
    let protocol = mode_name(mode);
    let request = read_control_with_retries(terminal, protocol)?;
    if request == CAN {
        return Err(TransferProtocolError::Canceled(protocol));
    }
    let uses_crc = match (mode, request) {
        (XMode::Crc, NAK) => false,
        (_, request) if request == mode.request() => mode.uses_crc(),
        _ => {
            return Err(malformed(
                protocol,
                "receiver requested a different checksum mode",
            ));
        }
    };
    if mode.streaming() && request != STREAM_REQUEST {
        return Err(malformed(
            protocol,
            "receiver requested a different checksum mode",
        ));
    }

    let mut block = 1_u8;
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = bytes.len().saturating_sub(offset);
        let block_size = if matches!(mode, XMode::OneK | XMode::OneKG) && remaining > 128 {
            1024
        } else {
            128
        };
        let count = remaining.min(block_size);
        let mut payload = vec![CPM_EOF; block_size];
        payload[..count].copy_from_slice(&bytes[offset..offset + count]);
        let packet = encode_block(block, &payload, uses_crc);
        send_block_with_retry(terminal, protocol, &packet, mode.streaming())?;
        offset += count;
        block = block.wrapping_add(1);
    }
    send_eot(terminal, protocol)
}

fn receive_xmodem(
    terminal: &mut dyn Terminal,
    fallback_name: &str,
    maximum_file_bytes: u64,
    mode: XMode,
) -> Result<Vec<ReceivedProtocolFile>, TransferProtocolError> {
    validate_remote_filename(fallback_name)?;
    let protocol = mode_name(mode);
    let mut request = mode.request();
    let mut uses_crc = mode.uses_crc();
    let mut attempts = 0;
    let first_control = loop {
        if attempts >= HANDSHAKE_RETRIES {
            return Err(TransferProtocolError::RetryLimit(protocol));
        }
        terminal.write_binary(&[request])?;
        match read_control_with_timeout(terminal, protocol, HANDSHAKE_TIMEOUT) {
            Ok(control) => break control,
            Err(TransferProtocolError::TimedOut(_)) => attempts += 1,
            Err(error) => return Err(error),
        }
        if matches!(mode, XMode::Crc) && uses_crc && attempts >= CRC_HANDSHAKE_RETRIES {
            // Ward Christensen's CRC extension permits a CRC receiver to
            // fall back after repeated unanswered `C` requests so it can
            // still interoperate with a checksum-only sender.
            uses_crc = false;
            request = NAK;
        }
    };
    let mut output = Vec::new();
    let mut expected = 1_u8;
    let mut retries = 0;
    let mut pending_control = Some(first_control);
    loop {
        let control = match pending_control.take() {
            Some(control) => control,
            None => read_control(terminal, protocol)?,
        };
        match control {
            EOT => {
                terminal.write_binary(&[ACK])?;
                break;
            }
            CAN => return Err(TransferProtocolError::Canceled(protocol)),
            SOH | STX => {
                let size = if control == SOH { 128 } else { 1024 };
                if control == STX && matches!(mode, XMode::Checksum | XMode::Crc) {
                    cancel(terminal)?;
                    return Err(malformed(protocol, "unexpected 1024-byte block"));
                }
                match read_and_validate_block(terminal, protocol, size, uses_crc) {
                    Ok((sequence, payload)) if sequence == expected => {
                        if output.len().saturating_add(payload.len()) as u64 > maximum_file_bytes {
                            cancel(terminal)?;
                            return Err(TransferProtocolError::TooLarge {
                                maximum: maximum_file_bytes,
                            });
                        }
                        output.extend_from_slice(&payload);
                        expected = expected.wrapping_add(1);
                        retries = 0;
                        if !mode.streaming() {
                            terminal.write_binary(&[ACK])?;
                        }
                    }
                    Ok((sequence, _)) if sequence == expected.wrapping_sub(1) => {
                        if !mode.streaming() {
                            terminal.write_binary(&[ACK])?;
                        }
                    }
                    Ok(_) | Err(_) if mode.streaming() => {
                        cancel(terminal)?;
                        return Err(malformed(protocol, "streaming block error"));
                    }
                    Ok(_) => {
                        cancel(terminal)?;
                        return Err(malformed(protocol, "out-of-sequence block"));
                    }
                    Err(error) => {
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            cancel(terminal)?;
                            return Err(error);
                        }
                        terminal.write_binary(&[NAK])?;
                    }
                }
            }
            _ => {
                retries += 1;
                if retries >= MAX_RETRIES {
                    cancel(terminal)?;
                    return Err(TransferProtocolError::RetryLimit(protocol));
                }
                if !mode.streaming() {
                    terminal.write_binary(&[request])?;
                }
            }
        }
    }
    Ok(vec![ReceivedProtocolFile {
        name: fallback_name.to_owned(),
        bytes: output,
        modified_unix: None,
    }])
}

fn send_ymodem(
    terminal: &mut dyn Terminal,
    files: &[ProtocolFile],
    streaming: bool,
) -> Result<(), TransferProtocolError> {
    let protocol = if streaming { "YMODEM-g" } else { "YMODEM" };
    let request = read_control_with_retries(terminal, protocol)?;
    if request
        != if streaming {
            STREAM_REQUEST
        } else {
            CRC_REQUEST
        }
    {
        return Err(malformed(protocol, "unexpected batch initiation"));
    }
    for file in files {
        validate_remote_filename(&file.name)?;
        let metadata = ymodem_metadata(file)?;
        send_ymodem_metadata(terminal, protocol, &metadata, request, streaming)?;
        if !streaming {
            let data_request = read_control_with_retries(terminal, protocol)?;
            if data_request != CRC_REQUEST {
                return Err(malformed(protocol, "receiver did not initiate file data"));
            }
        }
        send_ymodem_data(terminal, protocol, &file.bytes, streaming)?;
        finish_ymodem_file_send(terminal, protocol)?;
        let next_request = read_control_with_retries(terminal, protocol)?;
        if next_request
            != if streaming {
                STREAM_REQUEST
            } else {
                CRC_REQUEST
            }
        {
            return Err(malformed(
                protocol,
                "receiver did not request next metadata block",
            ));
        }
    }
    send_ymodem_metadata(terminal, protocol, &[0_u8; 128], request, streaming)
}

fn send_ymodem_metadata(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    metadata: &[u8],
    request: u8,
    streaming: bool,
) -> Result<(), TransferProtocolError> {
    if streaming {
        drain_repeated_control(terminal, protocol, request)?;
    }
    let packet = encode_block(0, metadata, true);
    if streaming && metadata[0] == 0 {
        terminal.write_binary(&packet)?;
        return Ok(());
    }
    for _ in 0..MAX_RETRIES {
        terminal.write_binary(&packet)?;
        for _ in 0..HANDSHAKE_RETRIES {
            match read_control_with_timeout(terminal, protocol, HANDSHAKE_TIMEOUT) {
                Ok(ACK) => return Ok(()),
                Ok(response) if streaming && response == STREAM_REQUEST => return Ok(()),
                Ok(CAN) => return Err(TransferProtocolError::Canceled(protocol)),
                Ok(NAK) => break,
                Ok(duplicate) if duplicate == request => {
                    // A terminal client may issue several receiver-driven
                    // initiation bytes while a human starts the sender. Those
                    // bytes can still be queued when block 0 is written. They
                    // are not a rejection and must not cause block 0 to be
                    // retransmitted into the following streaming phase.
                }
                Err(TransferProtocolError::TimedOut(_)) => continue,
                Ok(_) => return Err(malformed(protocol, "unexpected metadata response")),
                Err(error) => return Err(error),
            }
        }
    }
    Err(TransferProtocolError::RetryLimit(protocol))
}

fn drain_repeated_control(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    request: u8,
) -> Result<(), TransferProtocolError> {
    loop {
        match read_control_with_timeout(terminal, protocol, Duration::from_millis(20)) {
            Ok(byte) if byte == request => {}
            Ok(CAN) => return Err(TransferProtocolError::Canceled(protocol)),
            Ok(_) => return Err(malformed(protocol, "unexpected queued initiation byte")),
            Err(TransferProtocolError::TimedOut(_)) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn send_ymodem_data(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    bytes: &[u8],
    streaming: bool,
) -> Result<(), TransferProtocolError> {
    let mut block = 1_u8;
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        let size = if remaining > 128 { 1024 } else { 128 };
        let count = remaining.min(size);
        let mut payload = vec![CPM_EOF; size];
        payload[..count].copy_from_slice(&bytes[offset..offset + count]);
        send_block_with_retry(
            terminal,
            protocol,
            &encode_block(block, &payload, true),
            streaming,
        )?;
        offset += count;
        block = block.wrapping_add(1);
    }
    Ok(())
}

fn receive_ymodem(
    terminal: &mut dyn Terminal,
    maximum_file_bytes: u64,
    maximum_files: usize,
    streaming: bool,
) -> Result<Vec<ReceivedProtocolFile>, TransferProtocolError> {
    let protocol = if streaming { "YMODEM-g" } else { "YMODEM" };
    let request = if streaming {
        STREAM_REQUEST
    } else {
        CRC_REQUEST
    };
    let mut files = Vec::new();
    let first_control = request_control_with_retries(terminal, protocol, request)?;
    loop {
        let marker = if files.is_empty() {
            first_control
        } else {
            read_control(terminal, protocol)?
        };
        if marker == CAN {
            return Err(TransferProtocolError::Canceled(protocol));
        }
        if marker != SOH {
            return Err(malformed(protocol, "metadata must use a 128-byte block"));
        }
        let (sequence, metadata) = read_and_validate_block(terminal, protocol, 128, true)?;
        if sequence != 0 {
            return Err(malformed(protocol, "metadata sequence must be zero"));
        }
        if metadata[0] == 0 {
            if !streaming {
                terminal.write_binary(&[ACK])?;
            }
            break;
        }
        if files.len() >= maximum_files {
            cancel(terminal)?;
            return Err(malformed(
                protocol,
                "batch file count exceeds configured limit",
            ));
        }
        let (name, size, modified_unix) = parse_ymodem_metadata(&metadata)?;
        if size > maximum_file_bytes {
            cancel(terminal)?;
            return Err(TransferProtocolError::TooLarge {
                maximum: maximum_file_bytes,
            });
        }
        if streaming {
            terminal.write_binary(&[request])?;
        } else {
            terminal.write_binary(&[ACK, request])?;
        }
        let bytes = receive_ymodem_data(terminal, protocol, size, streaming)?;
        files.push(ReceivedProtocolFile {
            name,
            bytes,
            modified_unix,
        });
        terminal.write_binary(&[request])?;
    }
    Ok(files)
}

fn receive_ymodem_data(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    exact_size: u64,
    streaming: bool,
) -> Result<Vec<u8>, TransferProtocolError> {
    let capacity = usize::try_from(exact_size).map_err(|_| TransferProtocolError::TooLarge {
        maximum: usize::MAX as u64,
    })?;
    let mut output = Vec::with_capacity(capacity);
    let mut expected = 1_u8;
    loop {
        let marker = read_control(terminal, protocol)?;
        match marker {
            EOT => {
                if streaming {
                    terminal.write_binary(&[ACK])?;
                } else {
                    terminal.write_binary(&[NAK])?;
                    if read_control(terminal, protocol)? != EOT {
                        return Err(malformed(protocol, "expected repeated EOT"));
                    }
                    terminal.write_binary(&[ACK])?;
                }
                output.truncate(capacity);
                return Ok(output);
            }
            CAN => return Err(TransferProtocolError::Canceled(protocol)),
            SOH | STX => {
                let block_size = if marker == SOH { 128 } else { 1024 };
                match read_and_validate_block(terminal, protocol, block_size, true) {
                    Ok((sequence, payload)) if sequence == expected => {
                        output.extend_from_slice(&payload);
                        expected = expected.wrapping_add(1);
                        if !streaming {
                            terminal.write_binary(&[ACK])?;
                        }
                    }
                    Ok(_) | Err(_) if streaming => {
                        cancel(terminal)?;
                        return Err(malformed(protocol, "streaming data error"));
                    }
                    Ok(_) => {
                        cancel(terminal)?;
                        return Err(malformed(protocol, "out-of-sequence data block"));
                    }
                    Err(error) => {
                        terminal.write_binary(&[NAK])?;
                        return Err(error);
                    }
                }
            }
            _ => return Err(malformed(protocol, "unexpected data control byte")),
        }
    }
}

fn send_zmodem(
    terminal: &mut dyn Terminal,
    files: &[ProtocolFile],
) -> Result<(), TransferProtocolError> {
    let mut sender = Sender::new().map_err(zmodem_error)?;
    sender.set_streaming_window(usize::MAX);
    let mut index = 0;
    let mut started = false;
    let mut finishing = false;
    let mut session_complete = false;
    let mut wire = [0_u8; 4096];
    let mut pending_wire = Vec::new();
    loop {
        match sender.poll() {
            Action::WriteWire(bytes) => {
                let bytes = bytes.to_vec();
                terminal.write_binary(&bytes)?;
                sender.wire_written(bytes.len());
            }
            Action::ReadFile { offset, max_len } => {
                let start = offset.get() as usize;
                let end = start.saturating_add(max_len).min(files[index].bytes.len());
                sender
                    .submit_file(&files[index].bytes[start..end])
                    .map_err(zmodem_error)?;
            }
            Action::Event(Event::FileCompleted) => {
                index += 1;
                started = false;
            }
            Action::Event(Event::SessionCompleted) => {
                session_complete = true;
            }
            Action::Event(Event::Aborted) => return Err(TransferProtocolError::Canceled("ZMODEM")),
            Action::Event(Event::FileStarted(_)) => {}
            Action::Event(_) => {}
            Action::Idle => {
                if index < files.len() && !started {
                    let file = &files[index];
                    validate_remote_filename(&file.name)?;
                    let size = u32::try_from(file.bytes.len()).map_err(|_| {
                        TransferProtocolError::TooLarge {
                            maximum: u32::MAX as u64,
                        }
                    })?;
                    sender
                        .start_file(FileInfo::new(
                            file.name.as_bytes(),
                            Some(Position::new(size)),
                        ))
                        .map_err(zmodem_error)?;
                    started = true;
                } else if index == files.len() && !finishing {
                    sender.finish().map_err(zmodem_error)?;
                    finishing = true;
                } else if session_complete {
                    return Ok(());
                } else if !pending_wire.is_empty() {
                    let consumed = sender.submit_wire(&pending_wire).map_err(zmodem_error)?;
                    if consumed == 0 {
                        return Err(malformed("ZMODEM", "sender made no input progress"));
                    }
                    pending_wire.drain(..consumed);
                } else {
                    match terminal.read_binary(&mut wire, DEFAULT_TIMEOUT) {
                        Ok(0) => return Err(TransferProtocolError::Disconnected("ZMODEM")),
                        Ok(count) => pending_wire.extend_from_slice(&wire[..count]),
                        Err(TerminalError::TimedOut) => sender.timeout().map_err(zmodem_error)?,
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            _ => {}
        }
    }
}

fn receive_zmodem(
    terminal: &mut dyn Terminal,
    maximum_file_bytes: u64,
    maximum_files: usize,
) -> Result<Vec<ReceivedProtocolFile>, TransferProtocolError> {
    let mut receiver = Receiver::with_flow_control(0, true).map_err(zmodem_error)?;
    receiver.set_manual_file_accept(true);
    let mut files = Vec::new();
    let mut current: Option<ReceivedProtocolFile> = None;
    let mut wire = [0_u8; 4096];
    let mut pending_wire = Vec::new();
    let mut session_complete = false;
    let mut final_oo_consumed = false;
    loop {
        match receiver.poll() {
            Action::WriteWire(bytes) => {
                let bytes = bytes.to_vec();
                terminal.write_binary(&bytes)?;
                receiver.wire_written(bytes.len());
            }
            Action::WriteFile(bytes) => {
                let bytes = bytes.to_vec();
                let file = current
                    .as_mut()
                    .ok_or_else(|| malformed("ZMODEM", "data arrived before file metadata"))?;
                if file.bytes.len().saturating_add(bytes.len()) as u64 > maximum_file_bytes {
                    receiver.abort().map_err(zmodem_error)?;
                    return Err(TransferProtocolError::TooLarge {
                        maximum: maximum_file_bytes,
                    });
                }
                file.bytes.extend_from_slice(&bytes);
                receiver.file_written(bytes.len()).map_err(zmodem_error)?;
            }
            Action::Event(Event::FileStarted(info)) => {
                if files.len() >= maximum_files {
                    receiver.skip_file().map_err(zmodem_error)?;
                    continue;
                }
                let name = String::from_utf8_lossy(info.name).into_owned();
                validate_remote_filename(&name)?;
                if info
                    .size
                    .is_some_and(|size| u64::from(size.get()) > maximum_file_bytes)
                {
                    receiver.skip_file().map_err(zmodem_error)?;
                    continue;
                }
                current = Some(ReceivedProtocolFile {
                    name,
                    bytes: Vec::new(),
                    modified_unix: None,
                });
                receiver.accept_file_at(0).map_err(zmodem_error)?;
            }
            Action::Event(Event::FileCompleted) => {
                let file = current
                    .take()
                    .ok_or_else(|| malformed("ZMODEM", "completion without active file"))?;
                files.push(file);
            }
            Action::Event(Event::SessionCompleted) => {
                session_complete = true;
            }
            Action::Event(Event::Aborted) => return Err(TransferProtocolError::Canceled("ZMODEM")),
            Action::Event(_) => {}
            Action::Idle if !pending_wire.is_empty() => {
                let consumed = receiver.submit_wire(&pending_wire).map_err(zmodem_error)?;
                if consumed == 0 {
                    return Err(malformed("ZMODEM", "receiver made no input progress"));
                }
                pending_wire.drain(..consumed);
            }
            Action::Idle if session_complete && !final_oo_consumed => {
                let mut oo = [0_u8; 2];
                read_exact(terminal, "ZMODEM", &mut oo)?;
                if oo != *b"OO" {
                    return Err(malformed("ZMODEM", "missing final OO session terminator"));
                }
                final_oo_consumed = true;
            }
            Action::Idle if session_complete => return Ok(files),
            Action::Idle => match terminal.read_binary(&mut wire, DEFAULT_TIMEOUT) {
                Ok(0) => return Err(TransferProtocolError::Disconnected("ZMODEM")),
                Ok(count) => pending_wire.extend_from_slice(&wire[..count]),
                Err(TerminalError::TimedOut) => receiver.timeout().map_err(zmodem_error)?,
                Err(error) => return Err(error.into()),
            },
            _ => {}
        }
    }
}

fn send_telink(
    terminal: &mut dyn Terminal,
    file: &ProtocolFile,
) -> Result<(), TransferProtocolError> {
    validate_remote_filename(&file.name)?;
    let request = read_control_with_retries(terminal, "Telink")?;
    if !matches!(request, NAK | CRC_REQUEST) {
        return Err(malformed("Telink", "unexpected receiver initiation"));
    }
    let mut header = [0_u8; 128];
    let size = u32::try_from(file.bytes.len()).map_err(|_| TransferProtocolError::TooLarge {
        maximum: u32::MAX as u64,
    })?;
    header[..4].copy_from_slice(&size.to_le_bytes());
    let name = file.name.as_bytes();
    header[8..8 + name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
    header[24] = 0;
    header[25..36].copy_from_slice(b"SPITFIRE NG");
    header[41] = u8::from(request == CRC_REQUEST);
    let mut packet = Vec::with_capacity(134);
    packet.extend_from_slice(&[TELINK_HEADER, 0, 0xff]);
    packet.extend_from_slice(&header);
    if request == CRC_REQUEST {
        packet.extend_from_slice(&crc16_xmodem(&header).to_be_bytes());
    } else {
        packet.push(checksum(&header));
    }
    send_block_with_retry(terminal, "Telink", &packet, false)?;
    send_xmodem_data_after_handshake(terminal, &file.bytes, request == CRC_REQUEST, "Telink")
}

fn receive_telink(
    terminal: &mut dyn Terminal,
    maximum_file_bytes: u64,
) -> Result<Vec<ReceivedProtocolFile>, TransferProtocolError> {
    if request_control_with_retries(terminal, "Telink", CRC_REQUEST)? != TELINK_HEADER {
        return Err(malformed("Telink", "missing TeLink descriptor block"));
    }
    let (sequence, header) = read_and_validate_block(terminal, "Telink", 128, true)?;
    if sequence != 0 {
        return Err(malformed("Telink", "descriptor sequence must be zero"));
    }
    let size = u32::from_le_bytes(
        header[..4]
            .try_into()
            .map_err(|_| malformed("Telink", "descriptor length is truncated"))?,
    ) as u64;
    if size > maximum_file_bytes {
        cancel(terminal)?;
        return Err(TransferProtocolError::TooLarge {
            maximum: maximum_file_bytes,
        });
    }
    let end = header[8..24]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(16);
    let name = String::from_utf8_lossy(&header[8..8 + end]).into_owned();
    validate_remote_filename(&name)?;
    terminal.write_binary(&[ACK])?;
    let mut result = receive_xmodem_data_after_handshake(terminal, size, true, "Telink")?;
    result.truncate(size as usize);
    Ok(vec![ReceivedProtocolFile {
        name,
        bytes: result,
        modified_unix: None,
    }])
}

fn send_xmodem_data_after_handshake(
    terminal: &mut dyn Terminal,
    bytes: &[u8],
    crc: bool,
    protocol: &'static str,
) -> Result<(), TransferProtocolError> {
    let mut block = 1_u8;
    for chunk in bytes.chunks(128) {
        let mut payload = [CPM_EOF; 128];
        payload[..chunk.len()].copy_from_slice(chunk);
        send_block_with_retry(
            terminal,
            protocol,
            &encode_block(block, &payload, crc),
            false,
        )?;
        block = block.wrapping_add(1);
    }
    send_eot(terminal, protocol)
}

fn receive_xmodem_data_after_handshake(
    terminal: &mut dyn Terminal,
    exact_size: u64,
    crc: bool,
    protocol: &'static str,
) -> Result<Vec<u8>, TransferProtocolError> {
    let mut output = Vec::new();
    let mut expected = 1_u8;
    loop {
        match read_control(terminal, protocol)? {
            EOT => {
                terminal.write_binary(&[ACK])?;
                return Ok(output);
            }
            SOH => {
                let (sequence, payload) = read_and_validate_block(terminal, protocol, 128, crc)?;
                if sequence != expected {
                    cancel(terminal)?;
                    return Err(malformed(protocol, "out-of-sequence data block"));
                }
                if output.len().saturating_add(payload.len()) as u64
                    > exact_size.saturating_add(127)
                {
                    cancel(terminal)?;
                    return Err(malformed(protocol, "data exceeds declared size"));
                }
                output.extend_from_slice(&payload);
                expected = expected.wrapping_add(1);
                terminal.write_binary(&[ACK])?;
            }
            CAN => return Err(TransferProtocolError::Canceled(protocol)),
            _ => return Err(malformed(protocol, "unexpected data control byte")),
        }
    }
}

fn ymodem_metadata(file: &ProtocolFile) -> Result<Vec<u8>, TransferProtocolError> {
    if file.name.len() + 2 >= 128 {
        return Err(TransferProtocolError::UnsafeFilename(file.name.clone()));
    }
    let mut metadata = vec![0_u8; 128];
    metadata[..file.name.len()].copy_from_slice(file.name.as_bytes());
    let mut fields = file.bytes.len().to_string();
    if let Some(modified) = file.modified_unix {
        fields.push(' ');
        fields.push_str(&format!("{modified:o}"));
    }
    let start = file.name.len() + 1;
    metadata[start..start + fields.len()].copy_from_slice(fields.as_bytes());
    Ok(metadata)
}

fn parse_ymodem_metadata(
    metadata: &[u8],
) -> Result<(String, u64, Option<u64>), TransferProtocolError> {
    let name_end = metadata
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| malformed("YMODEM", "filename is not terminated"))?;
    let name = String::from_utf8_lossy(&metadata[..name_end]).into_owned();
    validate_remote_filename(&name)?;
    let fields = &metadata[name_end + 1..];
    let fields_end = fields
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(fields.len());
    let fields = String::from_utf8_lossy(&fields[..fields_end]);
    let mut parts = fields.split_ascii_whitespace();
    let size = parts
        .next()
        .ok_or_else(|| malformed("YMODEM", "file size is missing"))?
        .parse::<u64>()
        .map_err(|_| malformed("YMODEM", "file size is invalid"))?;
    let modified = parts
        .next()
        .map(|value| u64::from_str_radix(value, 8))
        .transpose()
        .map_err(|_| malformed("YMODEM", "modification date is invalid"))?;
    Ok((name, size, modified))
}

fn finish_ymodem_file_send(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
) -> Result<(), TransferProtocolError> {
    terminal.write_binary(&[EOT])?;
    match read_control(terminal, protocol)? {
        NAK => {
            terminal.write_binary(&[EOT])?;
            if read_control(terminal, protocol)? != ACK {
                return Err(malformed(protocol, "receiver did not acknowledge EOT"));
            }
        }
        ACK => {}
        CAN => return Err(TransferProtocolError::Canceled(protocol)),
        _ => return Err(malformed(protocol, "unexpected EOT response")),
    }
    Ok(())
}

fn send_eot(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
) -> Result<(), TransferProtocolError> {
    for _ in 0..MAX_RETRIES {
        terminal.write_binary(&[EOT])?;
        match read_control(terminal, protocol) {
            Ok(ACK) => return Ok(()),
            Ok(CAN) => return Err(TransferProtocolError::Canceled(protocol)),
            Ok(_) | Err(TransferProtocolError::TimedOut(_)) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(TransferProtocolError::RetryLimit(protocol))
}

fn send_block_with_retry(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    packet: &[u8],
    streaming: bool,
) -> Result<(), TransferProtocolError> {
    if streaming {
        terminal.write_binary(packet)?;
        return Ok(());
    }
    for _ in 0..MAX_RETRIES {
        terminal.write_binary(packet)?;
        match read_control(terminal, protocol) {
            Ok(ACK) => return Ok(()),
            Ok(NAK) | Err(TransferProtocolError::TimedOut(_)) => continue,
            Ok(CAN) => return Err(TransferProtocolError::Canceled(protocol)),
            Ok(_) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(TransferProtocolError::RetryLimit(protocol))
}

fn encode_block(sequence: u8, payload: &[u8], crc: bool) -> Vec<u8> {
    let mut block = Vec::with_capacity(payload.len() + 5);
    block.push(if payload.len() == 1024 { STX } else { SOH });
    block.push(sequence);
    block.push(!sequence);
    block.extend_from_slice(payload);
    if crc {
        block.extend_from_slice(&crc16_xmodem(payload).to_be_bytes());
    } else {
        block.push(checksum(payload));
    }
    block
}

fn read_and_validate_block(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    payload_size: usize,
    crc: bool,
) -> Result<(u8, Vec<u8>), TransferProtocolError> {
    let mut remainder = vec![0_u8; 2 + payload_size + usize::from(crc) + 1];
    read_exact(terminal, protocol, &mut remainder)?;
    let sequence = remainder[0];
    if remainder[1] != !sequence {
        return Err(malformed(protocol, "block sequence complement is invalid"));
    }
    let payload = remainder[2..2 + payload_size].to_vec();
    if crc {
        let received =
            u16::from_be_bytes([remainder[2 + payload_size], remainder[3 + payload_size]]);
        if received != crc16_xmodem(&payload) {
            return Err(malformed(protocol, "block CRC is invalid"));
        }
    } else if remainder[2 + payload_size] != checksum(&payload) {
        return Err(malformed(protocol, "block checksum is invalid"));
    }
    Ok((sequence, payload))
}

fn read_control(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
) -> Result<u8, TransferProtocolError> {
    read_control_with_timeout(terminal, protocol, DEFAULT_TIMEOUT)
}

fn read_control_with_timeout(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    timeout: Duration,
) -> Result<u8, TransferProtocolError> {
    let mut byte = [0_u8; 1];
    match terminal.read_binary(&mut byte, timeout) {
        Ok(0) => Err(TransferProtocolError::Disconnected(protocol)),
        Ok(_) => Ok(byte[0]),
        Err(TerminalError::TimedOut) => Err(TransferProtocolError::TimedOut(protocol)),
        Err(error) => Err(error.into()),
    }
}

fn request_control_with_retries(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    request: u8,
) -> Result<u8, TransferProtocolError> {
    for _ in 0..HANDSHAKE_RETRIES {
        terminal.write_binary(&[request])?;
        match read_control_with_timeout(terminal, protocol, HANDSHAKE_TIMEOUT) {
            Err(TransferProtocolError::TimedOut(_)) => continue,
            result => return result,
        }
    }
    Err(TransferProtocolError::RetryLimit(protocol))
}

fn read_control_with_retries(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
) -> Result<u8, TransferProtocolError> {
    for _ in 0..MAX_RETRIES {
        match read_control(terminal, protocol) {
            Err(TransferProtocolError::TimedOut(_)) => continue,
            result => return result,
        }
    }
    Err(TransferProtocolError::RetryLimit(protocol))
}

fn read_exact(
    terminal: &mut dyn Terminal,
    protocol: &'static str,
    mut output: &mut [u8],
) -> Result<(), TransferProtocolError> {
    while !output.is_empty() {
        match terminal.read_binary(output, DEFAULT_TIMEOUT) {
            Ok(0) => return Err(TransferProtocolError::Disconnected(protocol)),
            Ok(count) if count <= output.len() => output = &mut output[count..],
            Ok(_) => {
                return Err(malformed(
                    protocol,
                    "terminal returned an invalid byte count",
                ))
            }
            Err(TerminalError::TimedOut) => return Err(TransferProtocolError::TimedOut(protocol)),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn cancel(terminal: &mut dyn Terminal) -> Result<(), TransferProtocolError> {
    terminal.write_binary(&[CAN; 8])?;
    Ok(())
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().copied().fold(0_u8, u8::wrapping_add)
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0_u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn validate_remote_filename(name: &str) -> Result<(), TransferProtocolError> {
    if name.is_empty() || name.len() > MAX_FILE_NAME_BYTES || normalize_filename(name).is_err() {
        return Err(TransferProtocolError::UnsafeFilename(name.to_owned()));
    }
    Ok(())
}

const fn mode_name(mode: XMode) -> &'static str {
    match mode {
        XMode::Checksum => "XMODEM checksum",
        XMode::Crc => "XMODEM CRC",
        XMode::OneK => "1K-XMODEM",
        XMode::OneKG => "1K-XMODEM-g",
    }
}

fn malformed(protocol: &'static str, detail: &'static str) -> TransferProtocolError {
    TransferProtocolError::Malformed { protocol, detail }
}

fn zmodem_error(error: zmodem2::Error) -> TransferProtocolError {
    TransferProtocolError::Zmodem(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::mpsc;

    struct DuplexTerminal {
        sender: mpsc::Sender<Vec<u8>>,
        receiver: mpsc::Receiver<Vec<u8>>,
        pending: VecDeque<u8>,
    }

    impl Terminal for DuplexTerminal {
        fn info(&self) -> crate::TerminalInfo {
            crate::TerminalInfo::in_memory()
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
            self.write_binary(bytes)
        }

        fn read_line(&mut self, _maximum_bytes: usize) -> Result<Option<Vec<u8>>, TerminalError> {
            Err(TerminalError::BinaryUnsupported)
        }

        fn begin_binary_mode(&mut self) -> Result<(), TerminalError> {
            Ok(())
        }

        fn read_binary(
            &mut self,
            buffer: &mut [u8],
            timeout: Duration,
        ) -> Result<usize, TerminalError> {
            if self.pending.is_empty() {
                match self.receiver.recv_timeout(timeout) {
                    Ok(bytes) => self.pending.extend(bytes),
                    Err(mpsc::RecvTimeoutError::Timeout) => return Err(TerminalError::TimedOut),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                }
            }
            let count = buffer.len().min(self.pending.len());
            for byte in &mut buffer[..count] {
                *byte = self.pending.pop_front().unwrap();
            }
            Ok(count)
        }

        fn write_binary(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
            self.sender
                .send(bytes.to_vec())
                .map_err(|_| TerminalError::Disconnected)
        }
    }

    fn duplex() -> (DuplexTerminal, DuplexTerminal) {
        let (a_tx, a_rx) = mpsc::channel();
        let (b_tx, b_rx) = mpsc::channel();
        (
            DuplexTerminal {
                sender: a_tx,
                receiver: b_rx,
                pending: VecDeque::new(),
            },
            DuplexTerminal {
                sender: b_tx,
                receiver: a_rx,
                pending: VecDeque::new(),
            },
        )
    }

    fn round_trip(
        protocol: TransferProtocol,
        files: Vec<ProtocolFile>,
    ) -> Vec<ReceivedProtocolFile> {
        let (mut sending, mut receiving) = duplex();
        let fallback = files[0].name.clone();
        let expected_count = files.len();
        let sender_files = files.clone();
        let sender =
            std::thread::spawn(move || send_binary_files(&mut sending, protocol, &sender_files));
        let received = receive_binary_files(
            &mut receiving,
            protocol,
            &fallback,
            1_000_000,
            expected_count,
        )
        .unwrap();
        sender.join().unwrap().unwrap();
        received
    }

    #[test]
    fn crc16_matches_published_check_value() {
        assert_eq!(crc16_xmodem(b"123456789"), 0x31c3);
    }

    #[test]
    fn blocks_reject_corrupt_sequence_and_crc() {
        let payload = [0x55; 128];
        let mut block = encode_block(1, &payload, true);
        block[2] = 1;
        let mut terminal = crate::InMemoryTerminal::with_binary_input(block[1..].iter().copied());
        assert!(read_and_validate_block(&mut terminal, "XMODEM", 128, true).is_err());

        let mut block = encode_block(1, &payload, true);
        let last = block.len() - 1;
        block[last] ^= 0xff;
        let mut terminal = crate::InMemoryTerminal::with_binary_input(block[1..].iter().copied());
        assert!(read_and_validate_block(&mut terminal, "XMODEM", 128, true).is_err());
    }

    #[test]
    fn crc_sender_honors_checksum_fallback_request() {
        let mut terminal = crate::InMemoryTerminal::with_binary_input([NAK, ACK, ACK]);
        send_xmodem(&mut terminal, &[0x42], XMode::Crc).unwrap();
        let wire = terminal.output();
        assert_eq!(wire.len(), 133);
        assert_eq!(wire[0], SOH);
        assert_eq!(wire[131], checksum(&wire[3..131]));
        assert_eq!(wire[132], EOT);
    }

    #[test]
    fn empty_xmodem_transfer_sends_only_end_of_transmission() {
        let mut terminal = crate::InMemoryTerminal::with_binary_input([CRC_REQUEST, ACK]);
        send_xmodem(&mut terminal, &[], XMode::Crc).unwrap();
        assert_eq!(terminal.output(), &[EOT]);
    }

    #[test]
    fn ymodem_metadata_round_trips_exact_size_and_date() {
        let file = ProtocolFile {
            name: "TEST.BIN".to_owned(),
            bytes: vec![0; 1025],
            modified_unix: Some(1_700_000_000),
        };
        let metadata = ymodem_metadata(&file).unwrap();
        assert_eq!(
            parse_ymodem_metadata(&metadata).unwrap(),
            ("TEST.BIN".to_owned(), 1025, Some(1_700_000_000))
        );
    }

    #[test]
    fn remote_pathnames_are_rejected() {
        for name in ["../evil", "/tmp/evil", "C:\\EVIL.BIN", "sub/evil"] {
            assert!(validate_remote_filename(name).is_err(), "{name}");
        }
    }

    #[test]
    fn xmodem_variants_round_trip_binary_blocks() {
        let bytes = (0_u8..=255).collect::<Vec<_>>();
        for protocol in [
            TransferProtocol::XmodemChecksum,
            TransferProtocol::XmodemCrc,
            TransferProtocol::Xmodem1k,
            TransferProtocol::Xmodem1kG,
        ] {
            let received = round_trip(
                protocol,
                vec![ProtocolFile {
                    name: "ALLBYTE.BIN".to_owned(),
                    bytes: bytes.clone(),
                    modified_unix: None,
                }],
            );
            assert_eq!(&received[0].bytes[..bytes.len()], bytes);
        }
    }

    #[test]
    fn ymodem_modes_round_trip_batch_with_exact_lengths() {
        let files = vec![
            ProtocolFile {
                name: "ONE.BIN".to_owned(),
                bytes: vec![0xff; 129],
                modified_unix: Some(1_700_000_000),
            },
            ProtocolFile {
                name: "TWO.BIN".to_owned(),
                bytes: (0_u8..=255).cycle().take(1025).collect(),
                modified_unix: None,
            },
        ];
        for protocol in [
            TransferProtocol::YmodemBatch,
            TransferProtocol::YmodemGBatch,
        ] {
            let received = round_trip(protocol, files.clone());
            assert_eq!(received[0].bytes, files[0].bytes);
            assert_eq!(received[1].bytes, files[1].bytes);
        }
    }

    #[test]
    fn zmodem_round_trips_batch_and_all_byte_values() {
        let files = vec![
            ProtocolFile {
                name: "CONTROL.BIN".to_owned(),
                bytes: (0_u8..=255).cycle().take(4097).collect(),
                modified_unix: None,
            },
            ProtocolFile {
                name: "EMPTY.BIN".to_owned(),
                bytes: Vec::new(),
                modified_unix: None,
            },
        ];
        let received = round_trip(TransferProtocol::ZmodemBatch, files.clone());
        assert_eq!(received[0].bytes, files[0].bytes);
        assert_eq!(received[1].bytes, files[1].bytes);
    }

    #[test]
    fn concurrent_protocol_engines_keep_session_state_isolated() {
        let all_bytes = (0_u8..=255).collect::<Vec<_>>();
        let x_bytes = all_bytes.clone();
        let y_bytes = all_bytes.clone();
        let z_bytes = all_bytes.clone();

        let xmodem = std::thread::spawn(move || {
            round_trip(
                TransferProtocol::XmodemChecksum,
                vec![ProtocolFile {
                    name: "NODE1.BIN".to_owned(),
                    bytes: x_bytes,
                    modified_unix: None,
                }],
            )
        });
        let ymodem = std::thread::spawn(move || {
            round_trip(
                TransferProtocol::YmodemBatch,
                vec![
                    ProtocolFile {
                        name: "NODE2A.BIN".to_owned(),
                        bytes: y_bytes.clone(),
                        modified_unix: None,
                    },
                    ProtocolFile {
                        name: "NODE2B.BIN".to_owned(),
                        bytes: y_bytes,
                        modified_unix: None,
                    },
                ],
            )
        });
        let zmodem = std::thread::spawn(move || {
            round_trip(
                TransferProtocol::ZmodemBatch,
                vec![
                    ProtocolFile {
                        name: "NODE3A.BIN".to_owned(),
                        bytes: z_bytes.clone(),
                        modified_unix: None,
                    },
                    ProtocolFile {
                        name: "NODE3B.BIN".to_owned(),
                        bytes: z_bytes,
                        modified_unix: None,
                    },
                ],
            )
        });

        let mut canceled = crate::InMemoryTerminal::with_binary_input([CAN]);
        assert!(matches!(
            send_xmodem(&mut canceled, &[0x42], XMode::Crc),
            Err(TransferProtocolError::Canceled("XMODEM CRC"))
        ));

        assert_eq!(xmodem.join().unwrap()[0].bytes, all_bytes);
        for file in ymodem.join().unwrap() {
            assert_eq!(file.bytes, all_bytes);
        }
        for file in zmodem.join().unwrap() {
            assert_eq!(file.bytes, all_bytes);
        }
    }

    #[test]
    fn telink_descriptor_preserves_exact_file_size() {
        let file = ProtocolFile {
            name: "TELINK.BIN".to_owned(),
            bytes: (0_u8..=255).cycle().take(1025).collect(),
            modified_unix: None,
        };
        let received = round_trip(TransferProtocol::Telink, vec![file.clone()]);
        assert_eq!(received[0].bytes, file.bytes);
    }
}

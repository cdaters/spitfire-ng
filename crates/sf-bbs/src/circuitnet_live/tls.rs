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

//! Standard TLS 1.3, explicit certificate trust, and bounded socket I/O.
use super::*;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, ClientConnection, Connection, RootCertStore, ServerConfig, ServerConnection,
};
use std::io::{self, Read, Write};

pub(super) struct Socket {
    stream: TcpStream,
    started: Instant,
    deadline: Instant,
}
impl Socket {
    fn budget(&self) -> io::Result<Duration> {
        let end = self.deadline.min(self.started + Duration::from_secs(120));
        end.checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
    }
}
impl Read for Socket {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.budget()?))?;
        self.stream.read(b)
    }
}
impl Write for Socket {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.budget()?))?;
        self.stream.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
pub(super) struct Channel {
    pub conn: Connection,
    socket: Socket,
}
impl Read for Channel {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.conn.reader().read(b) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.conn.complete_io(&mut self.socket)?;
                }
                result => return result,
            }
        }
    }
}
impl Write for Channel {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.conn.writer().write(b)?;
        self.flush()?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        while self.conn.wants_write() {
            self.conn.complete_io(&mut self.socket)?;
        }
        Ok(())
    }
}
fn roots<'a>(certificates: impl Iterator<Item = &'a Vec<u8>>) -> Result<RootCertStore, Error> {
    let mut roots = RootCertStore::empty();
    for c in certificates {
        roots
            .add(CertificateDer::from(c.clone()))
            .map_err(|_| Error::Tls)?;
    }
    Ok(roots)
}
pub(super) fn validate_identity(certificate: &[u8], key: Vec<u8>) -> Result<(), Error> {
    ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| Error::Tls)?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(certificate.to_vec())],
            PrivateKeyDer::try_from(key).map_err(|_| Error::Tls)?,
        )
        .map_err(|_| Error::Tls)?;
    Ok(())
}
impl Channel {
    pub fn open(
        stream: TcpStream,
        config: &Config,
        peer: Option<&Peer>,
        key: Vec<u8>,
    ) -> Result<Self, Error> {
        // Accepted sockets may inherit O_NONBLOCK on some platforms. This host
        // uses deadline-bound blocking I/O, independently of listener mode.
        stream.set_nonblocking(false)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let key = PrivateKeyDer::try_from(key).map_err(|_| Error::Tls)?;
        let certs = vec![CertificateDer::from(config.certificate.clone())];
        let conn = if let Some(peer) = peer {
            let mut c = ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| Error::Tls)?
                .with_root_certificates(roots(std::iter::once(&peer.certificate))?)
                .with_client_auth_cert(certs, key)
                .map_err(|_| Error::Tls)?;
            c.alpn_protocols = vec![wire::ALPN.to_vec()];
            c.resumption = rustls::client::Resumption::disabled();
            Connection::Client(
                ClientConnection::new(
                    Arc::new(c),
                    ServerName::try_from(peer.server_name.clone()).map_err(|_| Error::Tls)?,
                )
                .map_err(|_| Error::Tls)?,
            )
        } else {
            let trust = roots(
                config
                    .peers
                    .iter()
                    .filter(|p| p.enabled && p.inbound && !p.held)
                    .map(|p| &p.certificate),
            )?;
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                Arc::new(trust),
                provider.clone(),
            )
            .build()
            .map_err(|_| Error::Tls)?;
            let mut c = ServerConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| Error::Tls)?
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
                .map_err(|_| Error::Tls)?;
            c.alpn_protocols = vec![wire::ALPN.to_vec()];
            c.send_tls13_tickets = 0;
            c.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
            Connection::Server(ServerConnection::new(Arc::new(c)).map_err(|_| Error::Tls)?)
        };
        let started = Instant::now();
        let socket = Socket {
            stream,
            started,
            deadline: started + Duration::from_secs(10),
        };
        let mut channel = Self { conn, socket };
        while channel.conn.is_handshaking() {
            channel.conn.complete_io(&mut channel.socket).map_err(|e| {
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) {
                    Error::Timeout
                } else {
                    Error::Tls
                }
            })?;
        }
        if channel.conn.alpn_protocol() != Some(wire::ALPN) {
            return Err(Error::UnsupportedVersion);
        }
        Ok(channel)
    }
    pub fn certificate(&self) -> Result<&[u8], Error> {
        self.conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| c.as_ref())
            .ok_or(Error::AuthFailed)
    }
    pub fn send(&mut self, f: &Frame) -> Result<usize, Error> {
        self.socket.deadline = Instant::now() + Duration::from_secs(10);
        wire::write(self, f)
    }
    pub fn receive(&mut self, limit: usize) -> Result<Frame, Error> {
        self.socket.deadline = Instant::now() + Duration::from_secs(10);
        wire::read(self, limit)
    }
    pub fn file_deadline(&mut self) {
        self.socket.deadline = Instant::now() + Duration::from_secs(90);
    }
    pub fn receive_file(&mut self) -> Result<Frame, Error> {
        self.file_deadline();
        wire::read(self, envelope::files::METADATA_FRAME)
    }
    pub fn close(&mut self) {
        self.conn.send_close_notify();
        let _ = self.flush();
    }
}

// From Saddle `plugins/dispatch/src/transport/tests/tls.rs` at commit `c21674a`, telemetry capture removed.
use super::*;
use rustls::{ServerConfig, ServerConnection};
use std::io::ErrorKind;
use std::sync::{Mutex, atomic::AtomicUsize};
use ureq::unversioned::{
    resolver::{ResolvedSocketAddrs, Resolver},
    transport::LazyBuffers,
};

#[derive(Debug, Default)]
struct Stats {
    plaintext: Mutex<Vec<u8>>,
    failed_writes: AtomicUsize,
}
#[derive(Debug)]
struct MemoryConnector {
    server: Arc<ServerConfig>,
    stats: Arc<Stats>,
    body_error: Option<ErrorKind>,
}
impl Connector for MemoryConnector {
    type Out = MemoryWire;
    fn connect(
        &self,
        _: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(Some(MemoryWire {
            server: ServerConnection::new(self.server.clone()).unwrap(),
            buffers: LazyBuffers::new(65536, 65536),
            stats: self.stats.clone(),
            body_error: self.body_error,
        }))
    }
}
#[derive(Debug)]
struct MemoryWire {
    server: ServerConnection,
    buffers: LazyBuffers,
    stats: Arc<Stats>,
    body_error: Option<ErrorKind>,
}
impl Transport for MemoryWire {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, n: usize, _: NextTimeout) -> Result<(), ureq::Error> {
        // Complete the real TLS handshake and HTTP headers, then reject body records.
        if let Some(kind) = self.body_error
            && self
                .stats
                .plaintext
                .lock()
                .unwrap()
                .windows(4)
                .any(|w| w == b"\r\n\r\n")
        {
            self.stats.failed_writes.fetch_add(1, Ordering::Relaxed);
            return Err(std::io::Error::from(kind).into());
        }
        self.server.read_tls(&mut &self.buffers.output()[..n])?;
        self.server.process_new_packets()?;
        if !self.server.is_handshaking() {
            let mut bytes = [0; 8192];
            loop {
                match self.server.reader().read(&mut bytes) {
                    Ok(0) => break,
                    Ok(n) => self
                        .stats
                        .plaintext
                        .lock()
                        .unwrap()
                        .extend_from_slice(&bytes[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e.into()),
                }
            }
        }
        Ok(())
    }
    fn await_input(&mut self, _: NextTimeout) -> Result<bool, ureq::Error> {
        let mut bytes = vec![];
        self.server.write_tls(&mut bytes)?;
        if bytes.is_empty() {
            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset).into());
        }
        assert!(bytes.len() <= self.buffers.input_append_buf().len());
        self.buffers.input_append_buf()[..bytes.len()].copy_from_slice(&bytes);
        self.buffers.input_appended(bytes.len());
        Ok(true)
    }
    fn is_open(&mut self) -> bool {
        true
    }
}
#[derive(Debug)]
struct NoDns;
impl Resolver for NoDns {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut addresses = self.empty();
        addresses.push("127.0.0.1:1".parse().unwrap());
        Ok(addresses)
    }
}

fn tls_failure(body_error: Option<ErrorKind>, message: &str) {
    let fail_body = body_error.is_some();
    // Public synthetic fixtures from ureq 3.4.2, src/unversioned/transport/testdata.
    let cert =
        ureq::tls::Certificate::from_pem(include_bytes!("../../../tests/fixtures/tls-cert.pem"))
            .unwrap();
    let key =
        ureq::tls::PrivateKey::from_pem(include_bytes!("../../../tests/fixtures/tls-key.pem"))
            .unwrap();
    let server = Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(cert.der().to_vec())],
                rustls::pki_types::PrivateKeyDer::try_from(key.der().to_vec()).unwrap(),
            )
            .unwrap(),
    );
    let mut evidence = vec![];
    let result = route::run(
        &mut &b"synthetic TLS send failure"[..],
        Some("synthetic-tls-key"),
        |body, key, _| {
            let stats = Arc::new(Stats::default());
            let sent = Arc::new(AtomicBool::new(false));
            let connector = MemoryConnector {
                server: server.clone(),
                stats: stats.clone(),
                body_error,
            }
            .chain(Observe {
                sent: sent.clone(),
                length: body.len(),
                uri: "https://synthetic.invalid/v1/systemone".parse().unwrap(),
            });
            // Fixture lifetime/identity only: no DNS/socket, no change to production TLS config.
            let cfg = Config::builder()
                .proxy(None)
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .disable_verification(true)
                        .build(),
                )
                .build();
            let raw = Agent::with_parts(cfg, connector, NoDns)
                .post("https://synthetic.invalid/v1/systemone")
                .header("Authorization", format!("Bearer {key}"))
                .header("Content-Type", "application/json")
                .send(body);
            let error = classify(raw.unwrap_err(), sent.load(Ordering::Relaxed));
            let plain = stats.plaintext.lock().unwrap();
            let end = plain.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let delivered = &plain[end..];
            // Establish injection/delivery before checking the regression outcome.
            if fail_body {
                assert!(stats.failed_writes.load(Ordering::Relaxed) > 0);
                assert!(delivered.is_empty());
            } else {
                assert_eq!(stats.failed_writes.load(Ordering::Relaxed), 0);
                assert_eq!(delivered, body);
            }
            evidence.push((
                body.len(),
                delivered.len(),
                sent.load(Ordering::Relaxed),
                error.retry,
            ));
            Err(error)
        },
        |_| panic!("network failure must not use status backoff"),
    );
    assert_eq!(result.exit_code, 1);
    assert!(!String::from_utf8_lossy(&result.stdout).contains("synthetic-tls-key"));
    let stdout: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(stdout["error"], message);
    assert_eq!(
        evidence.len(),
        if fail_body { 2 } else { 1 },
        "{evidence:?}"
    );
    for (_, _, sent, retry) in evidence {
        assert_eq!(sent, !fail_body);
        assert_eq!(retry, fail_body);
    }
}

#[test]
fn tls_hidden_body_write_failure_retries_once_with_one_begin() {
    tls_failure(
        Some(ErrorKind::BrokenPipe),
        "network: connection or send failed",
    );
}

#[test]
fn tls_hidden_body_write_timeout_keeps_timeout_classification() {
    tls_failure(Some(ErrorKind::TimedOut), "network: timeout");
}

#[test]
fn tls_receive_reset_after_complete_send_does_not_retry() {
    tls_failure(None, "network: response interrupted");
}

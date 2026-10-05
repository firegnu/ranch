// From Saddle `plugins/dispatch/src/transport.rs` at commit `c21674a`, telemetry capture removed.
//! ureq 3.4.2's unversioned adapter is confined here. Track successful wire writes,
//! not a drained body Reader or NextTimeout.reason (which need not name the phase).
use super::route::{Failure, MAX_RESPONSE, Response};
use std::{
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use ureq::{
    Agent,
    config::Config,
    unversioned::{
        resolver::DefaultResolver,
        transport::{
            Buffers, ConnectProxyConnector, ConnectionDetails, Connector, NextTimeout,
            RustlsConnector, TcpConnector, Transport,
        },
    },
};

pub(super) const URL: &str = "https://api.typesafe.ai/v1/systemone";
const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn post(body: &[u8], key: &str, first: bool) -> Result<Response, Failure> {
    send(URL, body, key, first, config(TIMEOUT))
}
fn config(timeout: Duration) -> Config {
    let builder = Config::builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(None)
        .timeout_per_call(None)
        .timeout_resolve(Some(timeout))
        .timeout_connect(Some(timeout))
        .timeout_send_request(Some(timeout))
        .timeout_send_body(Some(timeout))
        .timeout_recv_response(Some(timeout))
        .timeout_recv_body(Some(timeout));
    #[cfg(test)]
    let builder = builder.proxy(None);
    builder.build()
}
fn send(
    url: &str,
    body: &[u8],
    key: &str,
    first: bool,
    config: Config,
) -> Result<Response, Failure> {
    let sent = Arc::new(AtomicBool::new(false));
    // Preserve the CONNECT/TCP/rustls order of the enabled default transports,
    // inserting the write check below TLS (inside Observe).
    let connector = ConnectProxyConnector::default()
        .chain(TcpConnector::default())
        .chain(Observe {
            sent: sent.clone(),
            length: body.len(),
            uri: url.parse().expect("internal URL"),
        });
    // New agent for each attempt: no reused connection or internal stale-pool retry.
    let agent = Agent::with_parts(config, connector, DefaultResolver::default());
    let mut response = agent
        .post(url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .send(body)
        .map_err(|e| classify(e, sent.load(Ordering::Relaxed)))?;
    let status = response.status().as_u16();
    if first && matches!(status, 429 | 529) {
        // urllib does not read a first retryable HTTP error body either.
        return Ok(Response {
            status,
            body: vec![],
        });
    }
    let success = (200..300).contains(&status);
    let limit = if success { MAX_RESPONSE + 1 } else { 300 };
    let mut bytes = vec![];
    response
        .body_mut()
        .as_reader()
        .take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            let mut error = classify(e.into(), true);
            if !success {
                error.retry = false;
            }
            error
        })?;
    if success && bytes.len() > MAX_RESPONSE {
        return Err(Failure {
            message: "response exceeds 16 MiB",
            retry: false,
        });
    }
    Ok(Response {
        status,
        body: bytes,
    })
}
fn is_timeout(error: &ureq::Error) -> bool {
    matches!(error, ureq::Error::Timeout(_))
        || matches!(error,ureq::Error::Io(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock))
}
fn classify(error: ureq::Error, sent: bool) -> Failure {
    let timeout = is_timeout(&error);
    // Deliberately closed diagnostics: never format library errors, headers or keys.
    let message = if timeout {
        "network: timeout"
    } else {
        match error {
            ureq::Error::HostNotFound => "network: host not found",
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "network: TLS failed",
            ureq::Error::Http(_) => "network: invalid request",
            _ if sent => "network: response interrupted",
            _ => "network: connection or send failed",
        }
    };
    Failure {
        message,
        retry: !sent || timeout,
    }
}

#[derive(Debug)]
struct Observe {
    sent: Arc<AtomicBool>,
    length: usize,
    uri: ureq::http::Uri,
}
impl<T: Transport> Connector<T> for Observe {
    type Out = Observed<Box<dyn Transport>>;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<T>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let write_failure = Arc::new(Mutex::new(None));
        let checked = chained.map(|inner| WriteChecked {
            inner,
            failure: write_failure.clone(),
        });
        let tls = RustlsConnector::default().connect(details, checked)?;
        Ok(tls.map(|inner| Observed {
            inner: inner.boxed(),
            // Recursive CONNECT setup must not mark the target request as sent.
            sent: if details.uri == &self.uri {
                self.sent.clone()
            } else {
                Arc::new(AtomicBool::new(false))
            },
            write_failure,
            progress: Progress {
                matched: 0,
                headers: false,
                remaining: self.length,
            },
        }))
    }
}

#[derive(Debug)]
struct WriteChecked<T> {
    inner: T,
    failure: Arc<Mutex<Option<std::io::ErrorKind>>>,
}
impl<T: Transport> Transport for WriteChecked<T> {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let result = self.inner.transmit_output(amount, timeout);
        if let Err(error) = &result {
            *self.failure.lock().unwrap() = Some(if is_timeout(error) {
                std::io::ErrorKind::TimedOut
            } else {
                std::io::ErrorKind::Other
            });
        }
        result
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.inner.await_input(timeout)
    }
    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }
    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}
#[derive(Clone, Copy, Debug)]
struct Progress {
    matched: usize,
    headers: bool,
    remaining: usize,
}
impl Progress {
    fn advance(&mut self, bytes: &[u8]) {
        // Only four delimiter states and a byte count; no header/key is retained.
        for &b in bytes {
            if self.headers {
                self.remaining = self.remaining.saturating_sub(1);
            } else {
                self.matched = if b == b"\r\n\r\n"[self.matched] {
                    self.matched + 1
                } else {
                    usize::from(b == b'\r')
                };
                if self.matched == 4 {
                    self.headers = true;
                }
            }
        }
    }
    fn done(&self) -> bool {
        self.headers && self.remaining == 0
    }
}
#[derive(Debug)]
struct Observed<T> {
    inner: T,
    sent: Arc<AtomicBool>,
    write_failure: Arc<Mutex<Option<std::io::ErrorKind>>>,
    progress: Progress,
}
impl<T: Transport> Transport for Observed<T> {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let mut next = self.progress;
        next.advance(&self.inner.buffers().output()[..amount]);
        self.inner.transmit_output(amount, timeout)?;
        // rustls 0.23.45 Stream::write can accept plaintext while hiding a failed
        // complete_io write. Surface that failure before committing body progress.
        if let Some(kind) = self.write_failure.lock().unwrap().take() {
            return Err(std::io::Error::from(kind).into());
        }
        self.progress = next;
        self.sent.store(next.done(), Ordering::Relaxed);
        Ok(())
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.inner.await_input(timeout)
    }
    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }
    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}

#[cfg(test)]
mod tests;

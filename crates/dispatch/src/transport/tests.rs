use super::*;

mod tls;

#[test]
fn connect_and_timeout_errors_retry_but_receive_reset_does_not() {
    // The network boundary's stage marker must distinguish identical IO errors.
    let reset = || ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
    assert!(classify(reset(), false).retry);
    assert!(!classify(reset(), true).retry);
    assert!(classify(ureq::Error::Timeout(ureq::Timeout::RecvResponse), true).retry);
    assert!(classify(ureq::Error::Timeout(ureq::Timeout::RecvBody), true).retry);
    assert!(classify(ureq::Error::HostNotFound, false).retry);
    assert!(classify(ureq::Error::Tls("synthetic TLS failure"), false).retry);
}

use super::super::route;
use saddle_core_plugin::{Begin, Call, Captured, End, Recorder};
use std::{io::Write, net::TcpListener, thread, time::Instant};

#[derive(Default)]
struct Recording(Vec<Begin>);
impl Recorder for Recording {
    fn begin(&mut self, b: Begin) {
        self.0.push(b);
    }
}
const VALID: &str = r#"{"answers":{"tier":{"probabilities":{"0":1},"score":0,"confidence":1},"cross_data_model":{"noul":0},"visible":{"noul":0},"doc_only":{"noul":0}}}"#;
fn reply(status: u16, body: &[u8]) -> Vec<u8> {
    let mut bytes=format!("HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nConnection: close\r\nLocation: http://127.0.0.1:1/must-not-follow\r\n\r\n",body.len()).into_bytes();
    bytes.extend(body);
    bytes
}
// Every server is joined before assertions. Time limits also make a missing or extra
// retry observable without leaving threads or listeners behind after a failing test.
fn local(
    replies: Vec<(Vec<u8>, Duration)>,
    timeout: Duration,
) -> (
    saddle_core_plugin::Completion,
    Recording,
    Vec<Vec<u8>>,
    usize,
    Vec<Duration>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1/systemone", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut requests = vec![];
        for (response, pause) in replies {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut socket = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    _ => return requests,
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = vec![];
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                if socket.read_exact(&mut byte).is_err() {
                    return requests;
                }
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request.clone()).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            request.extend(body);
            requests.push(request);
            // Hold the connection past the injected deadline; no production timeout changes.
            if response.is_empty() && !pause.is_zero() {
                thread::sleep(pause);
            } else {
                let _ = socket.write_all(&response);
                thread::sleep(pause);
            }
        }
        requests
    });
    let mut rec = Recording::default();
    let mut attempts = 0;
    let mut sleeps = vec![];
    let result = route::run(
        Call {
            command: "route",
            stdin: &mut &b"synthetic summary"[..],
            recorder: &mut rec,
        },
        Some("synthetic-key-03c"),
        |body, key, first| {
            attempts += 1;
            send(&url, body, key, first, config(timeout))
        },
        |d| sleeps.push(d),
    );
    let requests = server.join().unwrap();
    (result, rec, requests, attempts, sleeps)
}
fn gap(result: saddle_core_plugin::Completion, expected: &str) {
    assert_eq!(result.exit_code, 1);
    let Some(End::Route(end)) = result.end else {
        panic!("end")
    };
    let actual = match end.response {
        Captured::Missing(Missing::NotAvailable) => "not_available",
        Captured::Missing(Missing::TooLarge) => "too_large",
        Captured::Missing(Missing::Unrecognized) => "unrecognized",
        _ => "unexpected body",
    };
    assert_eq!(actual, expected);
    let Captured::Bytes(suggestion) = end.suggestion else {
        panic!("suggestion")
    };
    assert_eq!(suggestion, result.stdout);
}
#[test]
fn loopback_observes_actual_request_retry_redirect_and_response_gaps() {
    for status in [429, 529] {
        let (result, rec, requests, attempts, sleeps) = local(
            vec![
                (reply(status, b"ignored"), Duration::ZERO),
                (reply(200, VALID.as_bytes()), Duration::ZERO),
            ],
            Duration::from_secs(1),
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(attempts, 2);
        assert_eq!(requests.len(), 2);
        assert_eq!(rec.0.len(), 1);
        assert_eq!(sleeps, vec![Duration::from_secs(1)]);
        let Begin::Route(begin) = &rec.0[0] else {
            panic!("begin")
        };
        for request in requests {
            let end = request.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
            let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
            assert!(headers.starts_with("post /v1/systemone http/1.1\r\n"));
            assert!(headers.contains("authorization: bearer synthetic-key-03c\r\n"));
            assert!(headers.contains("content-type: application/json\r\n"));
            assert_eq!(&request[end..], begin.request);
        }
    }
    for status in [302, 400, 503] {
        let (result, _, requests, attempts, sleeps) = local(
            vec![(
                reply(status, &[vec![b'x'; 299], vec![0xe4, 0xb8, 0xad]].concat()),
                Duration::ZERO,
            )],
            Duration::from_secs(1),
        );
        assert_eq!(attempts, 1);
        assert_eq!(requests.len(), 1);
        assert!(sleeps.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            value["error"],
            format!("HTTP {status}: {}�", "x".repeat(299))
        );
        gap(result, "not_available");
    }
    for (wire, expected) in [
        (reply(200, b"{not json}"), "unrecognized"),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n{}".to_vec(),
            "not_available",
        ),
        (vec![], "not_available"), // close after fully reading request, before headers
        (reply(200, &vec![b' '; MAX_RESPONSE + 1]), "too_large"),
    ] {
        let (result, _, requests, attempts, _) =
            local(vec![(wire, Duration::ZERO)], Duration::from_secs(1));
        assert_eq!(attempts, 1);
        assert_eq!(requests.len(), 1);
        gap(result, expected);
    }
}
#[test]
fn loopback_timeouts_retry_once_and_keep_response_unavailable() {
    for prefix in [
        vec![],
        b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n{".to_vec(),
    ] {
        let replies = vec![
            (prefix.clone(), Duration::from_millis(120)),
            (prefix, Duration::from_millis(120)),
        ];
        let (result, rec, requests, attempts, sleeps) = local(replies, Duration::from_millis(40));
        assert_eq!(attempts, 2);
        assert_eq!(rec.0.len(), 1);
        assert_eq!(requests.len(), 2);
        assert!(sleeps.is_empty());
        gap(result, "not_available");
    }
}
#[test]
fn production_transport_constants_have_phase_budgets_and_no_redirects() {
    assert_eq!(URL, "https://api.typesafe.ai/v1/systemone");
    let config = config(TIMEOUT);
    let t = config.timeouts();
    assert_eq!(config.max_redirects(), 0);
    assert!(!config.http_status_as_error());
    assert!(!config.tls_config().disable_verification());
    assert_eq!(t.global, None);
    assert_eq!(t.per_call, None);
    for value in [
        t.resolve,
        t.connect,
        t.send_request,
        t.send_body,
        t.recv_response,
        t.recv_body,
    ] {
        assert_eq!(value, Some(Duration::from_secs(10)));
    }
}

#[test]
fn send_marker_requires_successful_transport_writes_of_the_complete_body() {
    use ureq::unversioned::transport::LazyBuffers;
    #[derive(Debug)]
    struct Wire {
        buffers: LazyBuffers,
        fail: bool,
    }
    impl Transport for Wire {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }
        fn transmit_output(&mut self, _: usize, _: NextTimeout) -> Result<(), ureq::Error> {
            if self.fail {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into())
            } else {
                Ok(())
            }
        }
        fn await_input(&mut self, _: NextTimeout) -> Result<bool, ureq::Error> {
            Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset).into())
        }
        fn is_open(&mut self) -> bool {
            true
        }
    }
    let sent = Arc::new(AtomicBool::new(false));
    let mut wire = Observed {
        inner: Wire {
            buffers: LazyBuffers::new(1024, 1024),
            fail: false,
        },
        sent: sent.clone(),
        write_failure: Arc::new(Mutex::new(None)),
        progress: Progress {
            matched: 0,
            headers: false,
            remaining: 3,
        },
    };
    // A reason unrelated to the current phase must not turn a partial send into a receive.
    let timeout = NextTimeout {
        after: ureq::unversioned::transport::time::Duration::from_secs(1),
        reason: ureq::Timeout::RecvBody,
    };
    for bytes in [
        b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r".as_slice(),
        b"\na",
        b"b",
    ] {
        wire.buffers().output()[..bytes.len()].copy_from_slice(bytes);
        wire.transmit_output(bytes.len(), timeout).unwrap();
        assert!(!sent.load(Ordering::Relaxed));
    }
    wire.inner.fail = true;
    wire.buffers().output()[0] = b'c';
    let error = wire.transmit_output(1, timeout).unwrap_err();
    assert!(!sent.load(Ordering::Relaxed));
    assert!(classify(error, sent.load(Ordering::Relaxed)).retry);
    wire.inner.fail = false;
    wire.transmit_output(1, timeout).unwrap();
    assert!(sent.load(Ordering::Relaxed));
    let error = wire.await_input(timeout).unwrap_err();
    assert!(!classify(error, sent.load(Ordering::Relaxed)).retry);
}

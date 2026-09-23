//! M0.15.7e.6: HTTP framing and connect behaviour of the loopback client.
//!
//! Live Run 3 established that Ollama answers an HTTP/1.1 request with
//! `Transfer-Encoding: chunked` once the reply is larger than a couple of KB, which
//! leaves a hex chunk-size line in front of the JSON. This client is a deliberately
//! tiny raw-TCP one (no chunk decoder), so it speaks HTTP/1.0 and reads a
//! close-delimited body. These tests pin that with fake 127.0.0.1 listeners only:
//! no Ollama, no model, no inference.

use maia_briefing::LocalProviderFailure;
use maia_local_model::{LoopbackLocalProvider, MAXIMUM_HTTP_RESPONSE_BYTES};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// What the fake sends back for a request.
enum Reply {
    /// Exactly these bytes, whatever the request said.
    Raw(Vec<u8>),
    /// Behaves like Ollama: chunked for HTTP/1.1, close-delimited for HTTP/1.0.
    OllamaLike(String),
}

/// Reads one request (head + Content-Length body) from `stream`.
fn read_request(stream: &mut TcpStream) -> Option<Vec<u8>> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut request = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        request.extend_from_slice(&chunk[..n]);
        if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= end + 4 + length {
                return Some(request);
            }
        }
    }
}

fn chunked(body: &str) -> Vec<u8> {
    // Two chunks, as a server flushing a large body would send.
    let (a, b) = body.as_bytes().split_at(body.len() / 2);
    let mut out = Vec::new();
    for part in [a, b] {
        out.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"0\r\n\r\n");
    out
}

/// Serves one connection. Returns the address and the request head that arrived.
fn serve_once(reply: Reply, delay: Duration) -> (SocketAddr, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let Some(request) = read_request(&mut stream) else {
            return;
        };
        let end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&request[..end]).into_owned();
        let http10 = head.starts_with("POST /api/generate HTTP/1.0");
        let _ = tx.send(head);
        thread::sleep(delay);
        let bytes = match reply {
            Reply::Raw(bytes) => bytes,
            Reply::OllamaLike(body) => {
                if http10 {
                    let mut out =
                        b"HTTP/1.0 200 OK\r\nContent-Type: application/json; charset=utf-8\r\n\r\n"
                            .to_vec();
                    out.extend_from_slice(body.as_bytes());
                    out
                } else {
                    let mut out = b"HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
                    out.extend_from_slice(&chunked(&body));
                    out
                }
            }
        };
        let _ = stream.write_all(&bytes);
        // Dropping the stream closes it: the body is close-delimited.
    });
    (addr, rx)
}

fn provider(addr: SocketAddr) -> LoopbackLocalProvider {
    LoopbackLocalProvider::new(addr, "test-model").unwrap()
}

/// An Ollama-style envelope well above 4 KB, with unicode.
fn large_envelope(text: &str) -> String {
    serde_json::json!({
        "model": "test-model",
        "response": text,
        "done": true,
        "context": (0..1500).collect::<Vec<u32>>(),
    })
    .to_string()
}

fn raw(status_line: &str, body: &str) -> Reply {
    Reply::Raw(
        format!("{status_line}\r\nContent-Type: application/json\r\n\r\n{body}").into_bytes(),
    )
}

/// G. The generic request line is HTTP/1.0.
#[test]
fn the_generic_request_is_sent_as_http_1_0() {
    let (addr, head) = serve_once(
        raw("HTTP/1.0 200 OK", r#"{"response":"ok"}"#),
        Duration::ZERO,
    );
    assert_eq!(provider(addr).complete("p", 64), Ok("ok".to_owned()));
    let head = head.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        head.starts_with("POST /api/generate HTTP/1.0\r\n"),
        "request head was: {head}"
    );
}

/// H. A large (> 4 KB) close-delimited Ollama-style envelope parses correctly, and
/// the text comes back exactly.
#[test]
fn a_large_close_delimited_envelope_parses_exactly() {
    let text = "Źródła zachowują niezmienną treść. ".repeat(150);
    let envelope = large_envelope(&text);
    assert!(envelope.len() > 4 * 1024);
    let (addr, _) = serve_once(Reply::OllamaLike(envelope), Duration::ZERO);
    assert_eq!(provider(addr).complete("p", 2_048), Ok(text));
}

/// K. The fake reproduces the Run 3 failure mode for an HTTP/1.1 client (raw body
/// starts with a hex chunk-size line, which is not JSON), and the real client
/// avoids it by asking for HTTP/1.0. No chunk marker is ever handed to the parser.
#[test]
fn chunk_markers_never_reach_the_json_parser() {
    let envelope = large_envelope(&"x".repeat(6_000));

    // Control: what an HTTP/1.1 client would have received from the same server.
    let (addr, _) = serve_once(Reply::OllamaLike(envelope.clone()), Duration::ZERO);
    let mut stream = TcpStream::connect(addr).unwrap();
    let body = "{}";
    stream
        .write_all(
            format!(
                "POST /api/generate HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
    let mut received = String::new();
    stream.read_to_string(&mut received).unwrap();
    let after_head = received.split_once("\r\n\r\n").unwrap().1;
    assert!(
        serde_json::from_str::<serde_json::Value>(after_head).is_err(),
        "the control body starts with a chunk-size line: {:?}",
        &after_head[..8]
    );

    // The real client, against the same kind of server, succeeds.
    let (addr, _) = serve_once(Reply::OllamaLike(envelope), Duration::ZERO);
    assert_eq!(provider(addr).complete("p", 2_048), Ok("x".repeat(6_000)));

    // And a server that nonetheless sends chunk framing is NOT silently accepted:
    // there is no chunk decoder, so it is a classified failure, never a result.
    let (addr, _) = serve_once(
        Reply::Raw(
            [
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
                chunked(r#"{"response":"ok"}"#),
            ]
            .concat(),
        ),
        Duration::ZERO,
    );
    assert_eq!(
        provider(addr).complete("p", 64),
        Err(LocalProviderFailure::MalformedResponse)
    );
}

/// I. Both `HTTP/1.0 200` and `HTTP/1.1 200` close-delimited successes are accepted.
#[test]
fn both_http_1_0_and_1_1_200_close_delimited_responses_are_accepted() {
    for status in ["HTTP/1.0 200 OK", "HTTP/1.1 200 OK", "HTTP/1.1 200 "] {
        let (addr, _) = serve_once(raw(status, r#"{"response":"fine"}"#), Duration::ZERO);
        assert_eq!(
            provider(addr).complete("p", 64),
            Ok("fine".to_owned()),
            "{status}"
        );
    }
}

/// J. Anything else is not success, whatever the body looks like.
#[test]
fn other_statuses_are_not_success() {
    for status in [
        "HTTP/1.0 500 Internal Server Error",
        "HTTP/1.1 404 Not Found",
        "HTTP/1.1 200",          // no reason phrase separator
        "HTTP/1.1 2000 Strange", // not 200
        "HTTP/2 200 OK",
        "garbage",
    ] {
        let (addr, _) = serve_once(raw(status, r#"{"response":"looks fine"}"#), Duration::ZERO);
        assert_eq!(
            provider(addr).complete("p", 64),
            Err(LocalProviderFailure::ProviderError),
            "{status}"
        );
    }
}

/// A 200 whose body is not the expected envelope is still not a result.
#[test]
fn a_200_without_the_expected_envelope_is_malformed() {
    for body in [
        "not json",
        r#"{"no_response_field":1}"#,
        r#"{"response":7}"#,
    ] {
        let (addr, _) = serve_once(raw("HTTP/1.0 200 OK", body), Duration::ZERO);
        assert_eq!(
            provider(addr).complete("p", 64),
            Err(LocalProviderFailure::MalformedResponse),
            "{body}"
        );
    }
}

/// Connect is a different timing class from inference: a dead listener fails at
/// once as `Unavailable`, nowhere near the (long) response timeout.
#[test]
fn a_dead_listener_fails_fast_as_unavailable() {
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
        // listener dropped here: nothing accepts on this port
    };
    let started = Instant::now();
    assert_eq!(
        provider(addr).complete("p", 2_048),
        Err(LocalProviderFailure::Unavailable)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}

/// M0.15.7e.7, end to end: a real peer sending a response over the wired
/// `MAXIMUM_HTTP_RESPONSE_BYTES` bound is refused, never truncated and parsed as a
/// result, and the call does not wait anywhere near the (long) response timeout to
/// notice — the bound is enforced as bytes accumulate, not after the fact.
#[test]
fn an_oversized_response_is_refused_not_truncated_and_parsed() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let Ok((mut peer, _)) = listener.accept() else {
            return;
        };
        let _ = read_request(&mut peer);
        let _ = peer.write_all(b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n");
        let _ = peer.write_all(br#"{"response":""#);
        // Stream well past the bound; the client must stop long before this ends.
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..((MAXIMUM_HTTP_RESPONSE_BYTES / chunk.len()) + 4) {
            if peer.write_all(&chunk).is_err() {
                return;
            }
        }
    });
    let started = Instant::now();
    let got = provider(addr).complete("p", 2_048);
    let took = started.elapsed();
    assert_eq!(got, Err(LocalProviderFailure::ProviderError));
    assert!(
        took < Duration::from_secs(20),
        "must fail closed as bytes accumulate, not wait out the response timeout: {took:?}"
    );
}

/// A large but comfortably-under-the-bound response is not artificially limited
/// below `MAXIMUM_HTTP_RESPONSE_BYTES`.
#[test]
fn a_response_comfortably_under_the_bound_still_succeeds() {
    let text = "y".repeat(1_000_000);
    assert!(text.len() < MAXIMUM_HTTP_RESPONSE_BYTES / 2);
    let envelope = large_envelope(&text);
    let (addr, _) = serve_once(Reply::OllamaLike(envelope), Duration::ZERO);
    assert_eq!(provider(addr).complete("p", 2_048), Ok(text));
}

/// The response timeout is a ceiling, not a wait: a reply that arrives after a
/// short delay is returned right away, even for a 2048-token budget whose ceiling
/// is ~23 minutes.
#[test]
fn a_prompt_reply_is_returned_immediately_not_at_the_ceiling() {
    let (addr, _) = serve_once(
        raw("HTTP/1.0 200 OK", r#"{"response":"done"}"#),
        Duration::from_millis(400),
    );
    let started = Instant::now();
    assert_eq!(provider(addr).complete("p", 2_048), Ok("done".to_owned()));
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(400) && took < Duration::from_secs(10),
        "{took:?}"
    );
}

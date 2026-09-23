//! M0.15.7e.5: the output budget of a generic local-model completion is a
//! provider-level safety ceiling, separate from the Briefing product limit.
//!
//! Live Run 2 failed at 0 ms because a 2048-token request for a generic completion
//! was rejected against the Briefing-specific 1024 limit before any connection was
//! made. These tests pin the corrected contract WITHOUT a model or Ollama: a fake
//! loopback TCP listener stands in for the runtime, records what arrives, and lets
//! each test prove whether the request reached the transport at all.
//!
//! Nothing here contacts Ollama, a model or the network beyond 127.0.0.1:0.

use maia_briefing::{
    BriefingError, BriefingPacket, LocalModelProvider, LocalProviderFailure, MAXIMUM_OUTPUT_TOKENS,
    PacketEvidence, consult_and_validate,
};
use maia_local_model::{LoopbackLocalProvider, MAXIMUM_COMPLETION_OUTPUT_TOKENS};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    sync::mpsc,
    thread,
    time::Duration,
};

/// The Round Table contract's request budget at the time of Run 2. Used only as a
/// value here; nothing in this crate knows about Round Table.
const ROUND_TABLE_REQUEST_BUDGET: u32 = 2_048;

/// A one-shot fake runtime. Serves exactly one connection with `reply_body` as the
/// Ollama envelope and reports the JSON request body it received.
struct FakeRuntime {
    addr: SocketAddr,
    listener: TcpListener,
    received: mpsc::Receiver<serde_json::Value>,
}

impl FakeRuntime {
    fn start(reply_body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = listener.try_clone().unwrap();
        let (tx, received) = mpsc::channel();
        thread::spawn(move || {
            let Ok((mut stream, _)) = server.accept() else {
                return;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            let (header_end, length) = loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    return;
                }
                request.extend_from_slice(&chunk[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let length = header
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (end + 4, length);
                }
            };
            while request.len() < header_end + length {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..n]);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&request[header_end..header_end + length]).unwrap();
            let _ = tx.send(body);
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply_body.len(),
                reply_body
            );
            let _ = stream.write_all(reply.as_bytes());
        });
        Self {
            addr,
            listener,
            received,
        }
    }

    fn provider(&self) -> LoopbackLocalProvider {
        LoopbackLocalProvider::new(self.addr, "test-model").unwrap()
    }

    /// The request body the provider actually sent, if it connected at all.
    fn request(&self) -> Option<serde_json::Value> {
        self.received.recv_timeout(Duration::from_millis(500)).ok()
    }

    /// True if nothing ever connected: the call failed before the transport.
    fn never_connected(self) -> bool {
        // Wake the server thread so it can exit, then see whether a request came in.
        self.listener.set_nonblocking(true).unwrap();
        let connected = self
            .received
            .recv_timeout(Duration::from_millis(300))
            .is_ok();
        // Unblock the server's accept() (which only returns for a connection).
        let _ = std::net::TcpStream::connect(self.addr);
        !connected
    }
}

fn packet() -> BriefingPacket {
    BriefingPacket {
        id: "budget-contract".into(),
        workspace_id: "01900000-0000-7000-8000-000000000000".into(),
        objective: "State the supplied fact.".into(),
        instructions: "Use only supplied evidence and cite it.".into(),
        template_version: "briefing-v1".into(),
        response_contract: "claims JSON".into(),
        privacy_classification: "internal".into(),
        assurance: "A1".into(),
        routing_constraints: "local_loopback_only".into(),
        evidence: vec![PacketEvidence {
            citation_id: "e1".into(),
            artifact_id: "01900000-0000-7000-8000-000000000010".into(),
            content_hash: "a".repeat(64),
            excerpt: "MAIA local briefing is advisory only.".into(),
        }],
    }
}

const CLAIMS_REPLY: &str = r#"{"response":"{\"claims\":[{\"text\":\"fact\",\"class\":\"SupportedByEvidence\",\"citations\":[\"e1\"]}]}"}"#;

/// A. The Round Table budget (2048) is accepted by generic completion and reaches
/// the transport with exactly that `num_predict`. (Run 2: it was rejected at once.)
#[test]
fn a_generic_completion_accepts_2048_and_reaches_the_transport() {
    let runtime = FakeRuntime::start(r#"{"response":"hello"}"#);
    let got = runtime
        .provider()
        .complete("a free-text prompt", ROUND_TABLE_REQUEST_BUDGET);
    assert_eq!(got, Ok("hello".to_owned()));
    let request = runtime.request().expect("the request reached the runtime");
    assert_eq!(
        request["options"]["num_predict"],
        ROUND_TABLE_REQUEST_BUDGET
    );
    assert_eq!(request["prompt"], "a free-text prompt");
    assert!(
        request.get("format").is_none(),
        "free text: no schema constraint"
    );
}

/// The 2048 case is a consequence of a real ceiling, not a special case: the ceiling
/// covers it with room, and its exact value is accepted too.
#[test]
fn the_generic_ceiling_is_a_named_provider_limit_above_2048_and_is_itself_accepted() {
    // Compile-time: the ceiling covers the Round Table budget with room, and it is
    // not the Briefing limit (the two are independent).
    const _: () = assert!(MAXIMUM_COMPLETION_OUTPUT_TOKENS > ROUND_TABLE_REQUEST_BUDGET);
    const _: () = assert!(MAXIMUM_COMPLETION_OUTPUT_TOKENS > MAXIMUM_OUTPUT_TOKENS);
    let runtime = FakeRuntime::start(r#"{"response":"ok"}"#);
    assert_eq!(
        runtime
            .provider()
            .complete("p", MAXIMUM_COMPLETION_OUTPUT_TOKENS),
        Ok("ok".to_owned())
    );
    let request = runtime.request().unwrap();
    assert_eq!(
        request["options"]["num_predict"],
        MAXIMUM_COMPLETION_OUTPUT_TOKENS
    );
}

/// B. A zero budget is still refused, and before any connection is made.
#[test]
fn a_generic_completion_still_rejects_a_zero_budget_before_the_network() {
    let runtime = FakeRuntime::start(r#"{"response":"never"}"#);
    let got = runtime.provider().complete("p", 0);
    assert_eq!(got, Err(LocalProviderFailure::ProviderError));
    assert!(runtime.never_connected(), "rejected before TCP");
}

/// C. Above the provider's own ceiling is refused, before any connection is made.
#[test]
fn a_generic_completion_rejects_a_budget_above_the_provider_ceiling_before_the_network() {
    for over in [
        MAXIMUM_COMPLETION_OUTPUT_TOKENS + 1,
        MAXIMUM_COMPLETION_OUTPUT_TOKENS * 2,
        u32::MAX,
    ] {
        let runtime = FakeRuntime::start(r#"{"response":"never"}"#);
        let got = runtime.provider().complete("p", over);
        assert_eq!(got, Err(LocalProviderFailure::ProviderError), "{over}");
        assert!(runtime.never_connected(), "{over}: rejected before TCP");
    }
}

/// D. Briefing keeps its own 1024 limit: through the Briefing entry point AND on the
/// adapter's `consult` directly, even though the generic ceiling is larger.
#[test]
fn briefing_consult_remains_constrained_to_its_own_1024_limit() {
    assert_eq!(MAXIMUM_OUTPUT_TOKENS, 1_024, "the Briefing product limit");

    // Through the Briefing contract.
    let runtime = FakeRuntime::start(CLAIMS_REPLY);
    let got = consult_and_validate(&runtime.provider(), &packet(), MAXIMUM_OUTPUT_TOKENS + 1);
    assert!(matches!(got, Err(BriefingError::PacketTooLarge)));
    assert!(runtime.never_connected());

    // Directly on the adapter: 1025 is a valid generic budget but not a Briefing one.
    let runtime = FakeRuntime::start(CLAIMS_REPLY);
    let got = runtime
        .provider()
        .consult(&packet(), MAXIMUM_OUTPUT_TOKENS + 1);
    assert_eq!(got.err(), Some(LocalProviderFailure::ProviderError));
    assert!(
        runtime.never_connected(),
        "the Briefing cap is enforced before TCP"
    );

    let runtime = FakeRuntime::start(CLAIMS_REPLY);
    let got = runtime
        .provider()
        .consult(&packet(), ROUND_TABLE_REQUEST_BUDGET);
    assert_eq!(got.err(), Some(LocalProviderFailure::ProviderError));
    assert!(runtime.never_connected());

    let runtime = FakeRuntime::start(CLAIMS_REPLY);
    assert_eq!(
        runtime.provider().consult(&packet(), 0).err(),
        Some(LocalProviderFailure::ProviderError)
    );
    assert!(runtime.never_connected());
}

/// The Briefing limit itself is still accepted, with the schema constraint attached,
/// and validates end to end.
#[test]
fn briefing_consult_at_exactly_1024_still_works_end_to_end() {
    let runtime = FakeRuntime::start(CLAIMS_REPLY);
    let result =
        consult_and_validate(&runtime.provider(), &packet(), MAXIMUM_OUTPUT_TOKENS).unwrap();
    assert!(!result.execution_authority);
    assert_eq!(result.claims.len(), 1);
    let request = runtime.request().expect("reached the runtime");
    assert_eq!(request["options"]["num_predict"], MAXIMUM_OUTPUT_TOKENS);
    assert!(
        request.get("format").is_some(),
        "the claims schema constraint is still applied to consult"
    );
}

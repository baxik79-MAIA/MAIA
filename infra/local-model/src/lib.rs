//! Concrete loopback-only adapter. Runtime/model details stay outside Core.
#![forbid(unsafe_code)]
use maia_briefing::{
    BriefingPacket, LocalModelProvider, LocalProviderFailure, MAXIMUM_OUTPUT_TOKENS,
    RawProviderResponse,
};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

/// Loopback connect budget. A local runtime either accepts immediately or is not
/// running, so connection establishment is a different timing class from
/// inference: a dead listener must fail fast as `Unavailable` and must never wait
/// out the (long) response timeout below.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Response timeout: a FAILURE CEILING for one non-streaming completion, not an
// expected duration. A reply that arrives after 3 minutes is returned at 3
// minutes; the ceiling only decides when a reply that never comes is given up on.
//
// Local inference here is CPU-only and far slower than a remote API. One
// reference measurement of a generic free-text completion on the reference host
// (Ollama, a 4B Q4_K_M model, 100% CPU, `temperature` 0, `num_predict` 2048):
// 793 tokens generated in 170.86 s (4.64 tok/s), 137 prompt tokens in ~4.5 s,
// 4.9 s model load, 180.24 s in total. Earlier sessions on the same host measured
// generation as slow as 1.8 tok/s, so a single fast run is not a minimum.
//
// The ceiling is therefore derived from the REQUESTED output budget at a rate held
// below the slowest ever observed, and NOT capped at some smaller "expected" size:
//
//   timeout = MODEL_LOAD_ALLOWANCE
//           + prompt_tokens / PROMPT_TOKENS_PER_SECOND
//           + max_output_tokens / OUTPUT_TOKENS_PER_SECOND_FLOOR
//   clamped to MINIMUM_RESPONSE_TIMEOUT ..= MAXIMUM_RESPONSE_TIMEOUT
//
// For a 2048-token request with a ~700-byte body: 25 + 7 + 1365.3 = ~1397 s.
//
// MAXIMUM_RESPONSE_TIMEOUT (3600 s) is the finite, provider-owned hard bound. It
// is derived from this crate's own generic completion ceiling
// (`MAXIMUM_COMPLETION_OUTPUT_TOKENS` = 4096): 4096 / 1.5 = ~2731 s of output time,
// leaving ~870 s of the 3600 s for model load and substantial prompt processing.
// It does NOT promise that every accepted request finishes inside it: a request
// that does not is failed closed as `Timeout`, never retried, never substituted.
/// Allowance for a cold model load (measured 4.9-21 s across sessions).
const MODEL_LOAD_ALLOWANCE: Duration = Duration::from_secs(25);
/// Conservative prompt-evaluation rate (measured 25.7-31.3 tok/s).
const PROMPT_TOKENS_PER_SECOND: f64 = 25.0;
/// Conservative characters per token used to estimate the prompt (measured ~4.3-4.7).
const CHARS_PER_TOKEN: f64 = 4.0;
/// Output-rate floor, held BELOW the slowest rate observed anywhere (1.8 tok/s);
/// the reference run measured 4.64 tok/s. Provider performance policy, CPU-first.
const OUTPUT_TOKENS_PER_SECOND_FLOOR: f64 = 1.5;
/// Floor, so a tiny request still tolerates a cold load plus scheduling noise.
const MINIMUM_RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
/// Finite hard maximum; see the derivation above.
const MAXIMUM_RESPONSE_TIMEOUT: Duration = Duration::from_secs(3_600);

/// The response timeout for one request whose JSON body is `request_bytes` long and
/// that may generate up to `max_output_tokens`. The output term uses the FULL
/// requested budget. Saturating: clamped in floating point before conversion, so
/// no input can overflow or panic.
fn response_timeout(request_bytes: usize, max_output_tokens: u32) -> Duration {
    let prompt_tokens = request_bytes as f64 / CHARS_PER_TOKEN;
    let seconds = MODEL_LOAD_ALLOWANCE.as_secs_f64()
        + prompt_tokens / PROMPT_TOKENS_PER_SECOND
        + f64::from(max_output_tokens) / OUTPUT_TOKENS_PER_SECOND_FLOOR;
    Duration::from_secs_f64(seconds.clamp(
        MINIMUM_RESPONSE_TIMEOUT.as_secs_f64(),
        MAXIMUM_RESPONSE_TIMEOUT.as_secs_f64(),
    ))
}

// ---------------------------------------------------------------------------
// Absolute transport deadline (M0.15.7e.7).
//
// `response_timeout` above sizes ONE absolute deadline for the whole exchange, but
// `set_read_timeout`/`set_write_timeout` alone do not enforce it: each is a PER-CALL
// timeout, reset on every read/write. A peer that keeps making a little progress —
// one byte every few milliseconds, forever — never trips a per-call timeout and can
// hold the connection open past the claimed ceiling. The same is true of a peer that
// stops consuming request bytes: `write_all` blocks on backpressure with no bound of
// its own.
//
// `write_bounded` and `read_bounded` below fix ONE `Instant` deadline before the
// first byte and re-derive the REMAINING duration before every single syscall, never
// a fresh one. Progress never resets the deadline; only reaching it does. `send_prompt`
// establishes that instant once, right after the connection is made (connection
// establishment itself stays bounded separately by `CONNECT_TIMEOUT`), so the whole
// provider call is bounded by approximately `CONNECT_TIMEOUT + response_timeout(..)`
// plus small scheduling overhead — never more, however slowly-but-steadily a peer
// trickles bytes or stalls consumption.
//
// A transport completion (full request written, full response received, EOF
// observed) that lands AT OR AFTER the deadline is not success: it is `Timeout`,
// whatever bytes happen to already be in hand. Continued progress right up to the
// boundary does not extend it.
/// Read-buffer chunk size for the bounded read loop.
const READ_CHUNK_BYTES: usize = 16 * 1024;
/// Hard ceiling on the raw HTTP response (status line, headers and body together)
/// accumulated for one call. Transport safety, not a semantic output-token rule:
/// generic completion is capped at `MAXIMUM_COMPLETION_OUTPUT_TOKENS` (4096) tokens,
/// and every envelope measured so far is a few KB to low tens of KB even with Ollama's
/// `context` array attached; 8 MiB leaves a very large safety margin while still
/// bounding memory against a broken or hostile local peer. A response that would
/// exceed it is refused before more of it is read; it is never truncated and then
/// parsed as if it were the whole, valid answer.
pub const MAXIMUM_HTTP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Local Intelligence Runtime capability contract (M0.15.9).
//
// `LocalProviderFailure` (defined in `maia_briefing`, Core) is Briefing's own,
// already-shipped, narrow failure shape; `complete()`/`consult()` keep returning
// it unchanged, so no existing consumer sees any behavioral difference. This
// crate is the capability that OWNS local failure classification (per the
// Capability Mesh ownership rule), so `LocalIntelligenceFailure` below is the
// richer taxonomy defined and produced HERE, distinguishing cases the wire
// protocol already tells apart internally but which used to be collapsed into
// `LocalProviderFailure::ProviderError` before reaching any caller. `complete()`
// and `consult()` map it down (`to_provider_failure`, exhaustively, so a new
// variant here cannot silently reach an old caller as something they don't
// expect); `complete_detailed()` returns it directly, together with timing, for
// callers that want to distinguish these cases (e.g. future DEVELOPMENT_EVOLUTION
// evidence collection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalIntelligenceFailure {
    /// Connecting to the local runtime failed or was refused (not loopback, or
    /// nothing is listening). No wire exchange occurred.
    RuntimeUnavailable,
    /// The absolute deadline elapsed before the exchange completed.
    Timeout,
    /// The request could not be written to the runtime — a transport failure
    /// distinct from a timeout (e.g. a zero-byte write, or a non-timeout I/O
    /// error while writing).
    WriteFailure,
    /// The response exceeded the provider's own size bound
    /// ([`MAXIMUM_HTTP_RESPONSE_BYTES`]) and was refused before being read in
    /// full; never truncated and treated as complete.
    ResponseTooLarge,
    /// The runtime replied, but not with a successful HTTP status, or the
    /// request itself was invalid (e.g. an out-of-range output budget) and was
    /// refused before any network attempt — the same contract the runtime would
    /// also have rejected.
    ModelRejected,
    /// A reply was received in full but could not be parsed as the expected
    /// envelope, or was not valid UTF-8.
    MalformedResponse,
    /// A transport failure that does not fit any of the above (e.g. a
    /// non-timeout I/O error while reading).
    Other,
}

impl LocalIntelligenceFailure {
    /// Maps down to Briefing's existing, unchanged failure shape. Exhaustive on
    /// purpose: a newly added variant here is a compile error until this mapping
    /// says what it means for `complete()`/`consult()`'s existing callers,
    /// rather than silently reaching them as something they have never seen.
    fn to_provider_failure(self) -> LocalProviderFailure {
        match self {
            Self::RuntimeUnavailable => LocalProviderFailure::Unavailable,
            Self::Timeout => LocalProviderFailure::Timeout,
            Self::WriteFailure | Self::ResponseTooLarge | Self::ModelRejected | Self::Other => {
                LocalProviderFailure::ProviderError
            }
            Self::MalformedResponse => LocalProviderFailure::MalformedResponse,
        }
    }
}

/// A byte sink whose write timeout can be re-armed before every syscall. Implemented
/// for [`TcpStream`] in production; a fake implementor lets tests exercise a peer that
/// never consumes input without depending on real OS socket-buffer sizes.
trait TimedWrite: Write {
    fn set_write_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
}
impl TimedWrite for TcpStream {
    fn set_write_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        TcpStream::set_write_timeout(self, Some(timeout))
    }
}

/// A byte source whose read timeout can be re-armed before every syscall. Implemented
/// for [`TcpStream`] in production; a fake implementor lets tests exercise a peer that
/// trickles or stalls without a real second socket.
trait TimedRead: Read {
    fn set_read_timeout(&mut self, timeout: Duration) -> std::io::Result<()>;
}
impl TimedRead for TcpStream {
    fn set_read_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
        TcpStream::set_read_timeout(self, Some(timeout))
    }
}

/// A platform's way of reporting that a read/write timeout elapsed. Anything else is a
/// genuine transport failure, not a deadline.
fn is_timeout_like(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

/// Writes all of `buf` to `sink`, re-deriving the timeout from the SAME absolute
/// `deadline` before every syscall. A stalled peer (backpressure that never clears)
/// is bounded by the deadline, not by how many bytes it happens to accept; bytes
/// already accepted are transport progress within this one attempt, never a retry.
fn write_bounded<W: TimedWrite>(
    sink: &mut W,
    mut buf: &[u8],
    deadline: Instant,
) -> Result<(), LocalIntelligenceFailure> {
    while !buf.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(LocalIntelligenceFailure::Timeout);
        }
        sink.set_write_timeout(remaining)
            .map_err(|_| LocalIntelligenceFailure::WriteFailure)?;
        match sink.write(buf) {
            // A zero-byte write with a non-empty buffer is not progress; failing
            // safely here is simpler and just as fail-closed as looping on it.
            Ok(0) => return Err(LocalIntelligenceFailure::WriteFailure),
            Ok(n) => buf = &buf[n..],
            Err(e) if is_timeout_like(&e) => return Err(LocalIntelligenceFailure::Timeout),
            Err(_) => return Err(LocalIntelligenceFailure::WriteFailure),
        }
    }
    Ok(())
}

/// Reads until EOF or `deadline`, re-deriving the timeout before every syscall, never
/// buffering past `max_bytes`. SUCCESS (returning the accumulated bytes) requires EOF
/// to be observed strictly before the deadline; a peer that keeps trickling bytes right
/// up to the boundary is a `Timeout`, not a partial success, because continued progress
/// does not push the deadline out.
fn read_bounded<R: TimedRead>(
    source: &mut R,
    deadline: Instant,
    max_bytes: usize,
) -> Result<Vec<u8>, LocalIntelligenceFailure> {
    let mut out = Vec::new();
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(LocalIntelligenceFailure::Timeout);
        }
        source
            .set_read_timeout(remaining)
            .map_err(|_| LocalIntelligenceFailure::Other)?;
        match source.read(&mut chunk) {
            Ok(0) => {
                // EOF only completes transport successfully if the deadline has not
                // already passed; a read that blocked right up to it and then finally
                // saw EOF is still a timeout, not a late success.
                return if Instant::now() < deadline {
                    Ok(out)
                } else {
                    Err(LocalIntelligenceFailure::Timeout)
                };
            }
            Ok(n) => {
                if Instant::now() >= deadline {
                    return Err(LocalIntelligenceFailure::Timeout);
                }
                if out.len() + n > max_bytes {
                    return Err(LocalIntelligenceFailure::ResponseTooLarge);
                }
                out.extend_from_slice(&chunk[..n]);
            }
            Err(e) if is_timeout_like(&e) => return Err(LocalIntelligenceFailure::Timeout),
            Err(_) => return Err(LocalIntelligenceFailure::Other),
        }
    }
}

/// Writes the request and reads the whole close-delimited response, both bounded by
/// the SAME absolute `deadline`. Success requires the entire response (through EOF)
/// to have been received before the deadline; the UTF-8 decode is not a transport
/// failure and is reported as `MalformedResponse`, matching how this crate already
/// classifies every other shape of an unusable response.
fn perform_request(
    stream: &mut TcpStream,
    request: &[u8],
    deadline: Instant,
    max_response_bytes: usize,
) -> Result<String, LocalIntelligenceFailure> {
    write_bounded(stream, request, deadline)?;
    let bytes = read_bounded(stream, deadline, max_response_bytes)?;
    String::from_utf8(bytes).map_err(|_| LocalIntelligenceFailure::MalformedResponse)
}

/// Provider-level safety ceiling for the output budget of ONE generic text
/// Provider-level safety ceiling for the output budget of ONE generic text
/// completion. Owned by this capability (the loopback local-model transport): it
/// bounds what may be asked of the local runtime, and it is deliberately NOT the
/// Briefing product limit (`maia_briefing::MAXIMUM_OUTPUT_TOKENS`, 1024), which
/// only governs `consult`, nor a figure taken from any other consumer. A budget of
/// 0 or above this ceiling is refused before anything touches the network; the
/// ceiling is not a target and is not an expected output size.
pub const MAXIMUM_COMPLETION_OUTPUT_TOKENS: u32 = 4_096;
/// Facts about the capability's availability, reported at a point in time.
/// Available fields are provider-neutral: no Ollama-specific detail crosses
/// this boundary. Consumers render their own presentation from these facts;
/// this type carries no pre-formatted display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIntelligenceStatus {
    /// Whether the runtime accepted a bounded loopback connection just now.
    /// A momentary fact, not a guarantee about the next call.
    pub available: bool,
    /// The model identity this provider was constructed with (the model
    /// actually asked for, not necessarily confirmed loaded — the loopback
    /// protocol does not report that distinction; see `warm()`).
    pub model: String,
}

/// A successful generic completion, with timing alongside the text so a
/// caller can distinguish "the runtime is slow" from "the runtime is down"
/// without instrumenting the call site itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionOutcome {
    pub text: String,
    pub duration: Duration,
}

/// Bounded probe timeout for [`LoopbackLocalProvider::status`]. Short and
/// separate from [`CONNECT_TIMEOUT`] on purpose: a status check is read
/// frequently (e.g. a UI polling loop) and must never itself become a source
/// of latency; 250 ms was the value already validated at product-integration
/// level for exactly this purpose before this capability existed.
const STATUS_PROBE_TIMEOUT: Duration = Duration::from_millis(250);

pub struct LoopbackLocalProvider {
    endpoint: SocketAddr,
    model: String,
}
impl LoopbackLocalProvider {
    pub fn new(
        endpoint: SocketAddr,
        model: impl Into<String>,
    ) -> Result<Self, LocalProviderFailure> {
        let model = model.into();
        if !endpoint.ip().is_loopback() || model.trim().is_empty() {
            Err(LocalProviderFailure::Unavailable)
        } else {
            Ok(Self { endpoint, model })
        }
    }

    /// Bounded (250 ms) loopback availability probe. Never blocks a caller for
    /// longer than that, whatever the runtime's actual state; does not warm it
    /// and does not send a prompt.
    pub fn status(&self) -> LocalIntelligenceStatus {
        LocalIntelligenceStatus {
            available: TcpStream::connect_timeout(&self.endpoint, STATUS_PROBE_TIMEOUT).is_ok(),
            model: self.model.clone(),
        }
    }

    /// Asks the runtime to load the model into memory, without requesting any
    /// generation, and discards the reply. Fire-and-forget: runs on its own
    /// detached thread and any failure is ignored, because this is purely an
    /// optimization — a caller that does not warm, or whose warm attempt fails,
    /// still gets a correct (if slower) answer from `complete`/`consult`, which
    /// pay the load cost themselves if it was not already paid.
    ///
    /// Moved here from a Desktop-local free function (M0.15.9): warm-up is a
    /// Local Intelligence Runtime lifecycle responsibility, not a consumer's.
    pub fn warm(&self) {
        let endpoint = self.endpoint;
        let model = self.model.clone();
        std::thread::spawn(move || {
            let Ok(mut stream) = TcpStream::connect_timeout(&endpoint, CONNECT_TIMEOUT) else {
                return;
            };
            // An empty prompt makes Ollama load the model and return without
            // generating. Never send evidence or a real question here.
            let body =
                serde_json::json!({ "model": model, "prompt": "", "stream": false }).to_string();
            let request = format!(
                "POST /api/generate HTTP/1.0\r\nHost: {endpoint}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
            if std::io::Write::write_all(&mut stream, request.as_bytes()).is_ok() {
                let mut sink = String::new();
                let _ = std::io::Read::read_to_string(&mut stream, &mut sink);
            }
        });
    }
}
impl LoopbackLocalProvider {
    /// Send a prompt to the local runtime and return its raw text response
    /// with timing, with `format` as an optional Ollama structured-decoding
    /// constraint. This is the one place that owns the wire protocol (HTTP/1.0,
    /// close-delimited framing, envelope shape); `consult`, `complete` and
    /// `complete_detailed` all build a request and call this rather than
    /// duplicating it. Returns the capability-owned failure taxonomy directly;
    /// callers that need Briefing's narrower, already-shipped shape map it down
    /// with [`LocalIntelligenceFailure::to_provider_failure`].
    fn send_prompt(
        &self,
        prompt: &str,
        max: u32,
        format: Option<serde_json::Value>,
    ) -> Result<CompletionOutcome, LocalIntelligenceFailure> {
        let started = Instant::now();
        // Transport safety only. The Briefing contract's own, tighter limit is
        // enforced in `consult`, not here. Rejected before any network attempt,
        // as a request the runtime's own contract would also refuse.
        if max == 0 || max > MAXIMUM_COMPLETION_OUTPUT_TOKENS {
            return Err(LocalIntelligenceFailure::ModelRejected);
        }
        let mut body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "prompt": prompt,
            "options": {"num_predict": max, "temperature": 0}
        });
        if let Some(format) = format {
            body["format"] = format;
        }
        let body = body.to_string();
        let mut s = TcpStream::connect_timeout(&self.endpoint, CONNECT_TIMEOUT)
            .map_err(|_| LocalIntelligenceFailure::RuntimeUnavailable)?;
        // ONE absolute deadline, fixed right after the connection is made, governs
        // BOTH writing the request and reading the whole response (see the module
        // comment above `write_bounded`/`read_bounded`). Connection establishment
        // stays bounded separately, by `CONNECT_TIMEOUT`, above.
        let deadline = Instant::now() + response_timeout(body.len(), max);
        // HTTP/1.0 on purpose. This non-streaming client reads a close-delimited
        // response. Ollama answers an HTTP/1.1 request with
        // `Transfer-Encoding: chunked` once the reply is larger than a couple of KB,
        // which would leave hex chunk-size lines in the body handed to the JSON
        // parser; HTTP/1.0 cannot be answered with chunked framing. No chunk decoder.
        let request = format!(
            "POST /api/generate HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.endpoint,
            body.len(),
            body
        );
        let text = perform_request(
            &mut s,
            request.as_bytes(),
            deadline,
            MAXIMUM_HTTP_RESPONSE_BYTES,
        )?;
        if !(text.starts_with("HTTP/1.0 200 ") || text.starts_with("HTTP/1.1 200 ")) {
            return Err(LocalIntelligenceFailure::ModelRejected);
        }
        let json = text
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .ok_or(LocalIntelligenceFailure::MalformedResponse)?;
        let envelope: serde_json::Value =
            serde_json::from_str(json).map_err(|_| LocalIntelligenceFailure::MalformedResponse)?;
        let text = envelope
            .get("response")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or(LocalIntelligenceFailure::MalformedResponse)?;
        Ok(CompletionOutcome {
            text,
            duration: started.elapsed(),
        })
    }

    /// Ask the local model a free-text prompt and return its raw text
    /// response, with no output-schema constraint.
    ///
    /// Distinct from `consult`, which additionally constrains decoding to
    /// the briefing claims schema and parses the result into it. This is
    /// for callers that want the model's own words rather than a structured
    /// claims list — e.g. a Round Table adapter, which lives outside this
    /// crate so that this crate stays Round-Table-unaware. This method
    /// carries no Round Table (or any other consumer's) vocabulary, only
    /// "ask the local model a prompt." Kept exactly as it already behaved
    /// before M0.15.9: same signature, same `LocalProviderFailure` mapping for
    /// every input that was already tested, proven by `to_provider_failure`
    /// being total and by the unchanged test suite.
    pub fn complete(
        &self,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<String, LocalProviderFailure> {
        self.send_prompt(prompt, max_output_tokens, None)
            .map(|outcome| outcome.text)
            .map_err(LocalIntelligenceFailure::to_provider_failure)
    }

    /// Like `complete`, but returns the capability-owned failure taxonomy and
    /// the call's timing instead of Briefing's narrower shape. For callers that
    /// want to distinguish e.g. a stalled write from an oversized response, or
    /// record latency — future DEVELOPMENT_EVOLUTION evidence collection, not
    /// product behavior change: nothing here alters what `complete`/`consult`
    /// return for the same input.
    pub fn complete_detailed(
        &self,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<CompletionOutcome, LocalIntelligenceFailure> {
        self.send_prompt(prompt, max_output_tokens, None)
    }
}
impl LocalModelProvider for LoopbackLocalProvider {
    fn requested_provider(&self) -> &str {
        "local-loopback"
    }
    fn requested_model(&self) -> &str {
        &self.model
    }
    fn consult(
        &self,
        packet: &BriefingPacket,
        max: u32,
    ) -> Result<RawProviderResponse, LocalProviderFailure> {
        let format = serde_json::json!({"type":"object","properties":{"claims":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"class":{"type":"string","enum":["SupportedByEvidence","Inference","UnknownNotSupported"]},"citations":{"type":"array","items":{"type":"string"}}},"required":["text","class","citations"],"additionalProperties":false}}},"required":["claims"],"additionalProperties":false});
        let prompt = format!(
            "You are an advisory evidence briefer. Return JSON only. Every SupportedByEvidence claim must cite one or more supplied citation IDs. Do not invent citations. Do not propose actions or execution. Objective: {}\nEvidence:\n{}",
            packet.objective,
            packet
                .evidence
                .iter()
                .map(|e| format!("[{}] {}", e.citation_id, e.excerpt))
                .collect::<Vec<_>>()
                .join("\n")
        );
        // The Briefing product limit stays a Briefing limit: `consult` keeps it even
        // though the generic completion ceiling above is larger.
        if max == 0 || max > MAXIMUM_OUTPUT_TOKENS {
            return Err(LocalProviderFailure::ProviderError);
        }
        let response = self
            .send_prompt(&prompt, max, Some(format))
            .map_err(LocalIntelligenceFailure::to_provider_failure)?;
        serde_json::from_str(&response.text).map_err(|_| LocalProviderFailure::MalformedResponse)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Connect is its own, short timing class: never the inference timeout. (A refused
    // loopback connection returns at once, so the value cannot be observed through a
    // socket; it is pinned here instead.)
    const _: () = assert!(CONNECT_TIMEOUT.as_secs() <= 5);
    const _: () = assert!(CONNECT_TIMEOUT.as_secs() < MINIMUM_RESPONSE_TIMEOUT.as_secs());

    /// A typical request body: a short prompt plus the JSON envelope, ~700 bytes.
    const TYPICAL_BODY: usize = 700;
    /// The reference measurement of the 2048-token completion: total wall time.
    const MEASURED_REFERENCE_SECONDS: f64 = 180.27;

    /// A. The output term uses the FULL requested budget, not a smaller cap.
    #[test]
    fn the_output_term_uses_the_full_requested_budget() {
        let full = response_timeout(TYPICAL_BODY, 2_048).as_secs_f64();
        let small = response_timeout(TYPICAL_BODY, 300).as_secs_f64();
        let expected_extra = (2_048.0 - 300.0) / OUTPUT_TOKENS_PER_SECOND_FLOOR;
        assert!(
            ((full - small) - expected_extra).abs() < 1.0,
            "2048 must be sized as 2048, not 300: {full} vs {small}"
        );
        // 25 s load + ~7 s prompt + 2048 / 1.5 s.
        assert!((1_390.0..=1_410.0).contains(&full), "got {full}");
    }

    /// B. Materially above the measured reference run: a ceiling, not the expected time.
    #[test]
    fn the_2048_ceiling_is_far_above_the_measured_reference_run() {
        let t = response_timeout(TYPICAL_BODY, 2_048).as_secs_f64();
        assert!(t > 7.0 * MEASURED_REFERENCE_SECONDS, "got {t}");
        // And a slow-end run (1.8 tok/s, 793 tokens = ~441 s) also fits.
        assert!(t > 793.0 / 1.8);
    }

    /// C. Monotonic in the requested budget (no clamp boundary is reached here).
    #[test]
    fn the_timeout_grows_with_the_requested_budget() {
        let t = |max| response_timeout(TYPICAL_BODY, max);
        assert!(t(512) < t(1_024) && t(1_024) < t(2_048) && t(2_048) < t(4_096));
        // And with the prompt.
        assert!(response_timeout(40_000, 2_048) > response_timeout(700, 2_048));
    }

    /// D. Finite: the ceiling for the provider's own maximum budget, and for absurd
    /// inputs, never exceeds the hard maximum; nor does it fall below the floor.
    #[test]
    fn the_timeout_is_finite_and_clamped() {
        assert_eq!(MAXIMUM_RESPONSE_TIMEOUT, Duration::from_secs(3_600));
        let at_provider_max = response_timeout(TYPICAL_BODY, MAXIMUM_COMPLETION_OUTPUT_TOKENS);
        assert!(at_provider_max <= MAXIMUM_RESPONSE_TIMEOUT);
        assert!(
            at_provider_max.as_secs_f64() > 4_096.0 / OUTPUT_TOKENS_PER_SECOND_FLOOR,
            "the derivation holds: the full output term fits inside the maximum"
        );
        assert_eq!(
            response_timeout(usize::MAX, u32::MAX),
            MAXIMUM_RESPONSE_TIMEOUT
        );
        assert_eq!(response_timeout(0, 1), MINIMUM_RESPONSE_TIMEOUT);
    }

    /// The old 300-token cap must not reappear in the generic provider.
    #[test]
    fn no_expected_output_cap_exists_in_the_generic_policy() {
        assert!(response_timeout(TYPICAL_BODY, 1_024) > response_timeout(TYPICAL_BODY, 300));
        assert!(response_timeout(TYPICAL_BODY, 2_048) > response_timeout(TYPICAL_BODY, 1_024));
    }

    // -------------------------------- absolute transport deadline (M0.15.7e.7)

    /// Never blocks: reports the same `WouldBlock` backpressure an un-drained TCP
    /// send buffer would, after accepting up to `allowed` bytes total. Deterministic,
    /// independent of any real OS socket-buffer size — exactly the seam the milestone
    /// asks for instead of a flaky real-backpressure timing test.
    struct StalledWriter {
        allowed: usize,
        consumed: usize,
    }
    impl Write for StalledWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.consumed >= self.allowed {
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
            let n = buf.len().min(self.allowed - self.consumed);
            self.consumed += n;
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl TimedWrite for StalledWriter {
        fn set_write_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A source that always reports the same `WouldBlock` a peer that stopped
    /// sending would; deterministic partner to `StalledWriter` for the read loop.
    struct StalledReader;
    impl Read for StalledReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
        }
    }
    impl TimedRead for StalledReader {
        fn set_read_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A sink that always accepts everything it is given, immediately. Used to prove
    /// the deadline CHECK itself gates the write — not merely that a write error maps
    /// to `Timeout` — by making a write that would otherwise trivially succeed.
    struct AcceptingWriter(Vec<u8>);
    impl Write for AcceptingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl TimedWrite for AcceptingWriter {
        fn set_write_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// If the absolute deadline has already passed before `write_bounded` is even
    /// called, not one byte may be written, however willing the peer is to accept
    /// them. This isolates the deadline check from write-error mapping: a mutation
    /// that drops the pre-write deadline check would let this succeed instead.
    #[test]
    fn write_bounded_refuses_even_the_first_byte_once_the_deadline_has_already_passed() {
        let deadline = Instant::now();
        std::thread::sleep(Duration::from_millis(5));
        let mut sink = AcceptingWriter(Vec::new());
        let got = write_bounded(&mut sink, b"consultation prompt", deadline);
        assert_eq!(got, Err(LocalIntelligenceFailure::Timeout));
        assert!(
            sink.0.is_empty(),
            "no byte should reach an already-expired call"
        );
    }

    /// B. A peer that accepts the connection but stops consuming request bytes must
    /// not hold the write loop open past the deadline: it must first make bounded,
    /// non-retried progress (multiple `write` calls succeed), then the deadline wins
    /// once the peer stalls, and it must never wait anywhere near a real timeout.
    #[test]
    fn the_write_loop_bounds_a_peer_that_stops_consuming_input() {
        let mut sink = StalledWriter {
            allowed: 1_000,
            consumed: 0,
        };
        let deadline = Instant::now() + Duration::from_millis(200);
        let started = Instant::now();
        let got = write_bounded(&mut sink, &[b'x'; 50_000], deadline);
        let took = started.elapsed();
        assert_eq!(got, Err(LocalIntelligenceFailure::Timeout));
        assert_eq!(
            sink.consumed, 1_000,
            "partial progress happened; not a retry"
        );
        assert!(
            took < Duration::from_secs(2),
            "must not hold the caller open: took {took:?}"
        );
    }

    /// A stalled peer during the READ phase is bounded the same way.
    #[test]
    fn the_read_loop_bounds_a_peer_that_never_sends_anything() {
        let deadline = Instant::now() + Duration::from_millis(150);
        let started = Instant::now();
        let got = read_bounded(&mut StalledReader, deadline, MAXIMUM_HTTP_RESPONSE_BYTES);
        assert_eq!(got, Err(LocalIntelligenceFailure::Timeout));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// C. The response-size bound is enforced exactly, not merely approximately: one
    /// byte over is refused, and the exact bound is still accepted.
    #[test]
    fn read_bounded_enforces_the_exact_response_size_bound() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        /// Yields bytes indefinitely in fixed-size bursts, deterministic and
        /// independent of real network timing.
        struct Firehose(Arc<AtomicUsize>);
        impl Read for Firehose {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.0.fetch_add(1, Ordering::SeqCst);
                let n = buf.len().min(4_096);
                for b in &mut buf[..n] {
                    *b = b'x';
                }
                Ok(n)
            }
        }
        impl TimedRead for Firehose {
            fn set_read_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
                Ok(())
            }
        }

        let deadline = Instant::now() + Duration::from_secs(30);
        let calls = Arc::new(AtomicUsize::new(0));
        let got = read_bounded(&mut Firehose(Arc::clone(&calls)), deadline, 10_000);
        assert_eq!(got, Err(LocalIntelligenceFailure::ResponseTooLarge));
        assert!(
            calls.load(Ordering::SeqCst) * 4_096 < 100_000,
            "must fail closed near the bound, not after unbounded accumulation"
        );

        /// A source that yields exactly `total` bytes, then EOF.
        struct Fixed {
            remaining: usize,
        }
        impl Read for Fixed {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = buf.len().min(self.remaining).min(4_096);
                self.remaining -= n;
                for b in &mut buf[..n] {
                    *b = b'x';
                }
                Ok(n)
            }
        }
        impl TimedRead for Fixed {
            fn set_read_timeout(&mut self, _timeout: Duration) -> std::io::Result<()> {
                Ok(())
            }
        }
        // Exactly at the bound: accepted.
        let got = read_bounded(&mut Fixed { remaining: 1_000 }, deadline, 1_000);
        assert_eq!(got.map(|b| b.len()), Ok(1_000));
        // One byte over: refused.
        let got = read_bounded(&mut Fixed { remaining: 1_001 }, deadline, 1_000);
        assert_eq!(got, Err(LocalIntelligenceFailure::ResponseTooLarge));
    }

    /// A. THE EXACT CODEX FAILURE — TRICKLE RESPONSE. A real loopback peer sends
    /// small fragments, each one comfortably inside a per-call read timeout, but the
    /// stream as a whole runs well past the absolute deadline. `set_read_timeout`
    /// alone (reset on every call) would never trip; the absolute deadline must. This
    /// test fails if the implementation reverts to per-read timeout semantics: it
    /// would then wait out the trickle's full, much longer duration instead of
    /// stopping at the deadline.
    #[test]
    fn a_real_peer_trickling_bytes_past_the_absolute_deadline_times_out() {
        use std::net::{TcpListener, TcpStream as StdStream};
        use std::thread;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Trickles far longer than the client's absolute deadline below, one byte at
        // a time, well inside any sane per-call timeout.
        let server = thread::spawn(move || {
            let Ok((mut peer, _)) = listener.accept() else {
                return;
            };
            for _ in 0..40 {
                if peer.write_all(b"x").is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(40)); // 40 x 40ms = 1.6s of trickle
            }
        });
        let mut client = StdStream::connect(addr).unwrap();
        let deadline = Instant::now() + Duration::from_millis(300);
        let started = Instant::now();
        let got = read_bounded(&mut client, deadline, MAXIMUM_HTTP_RESPONSE_BYTES);
        let took = started.elapsed();
        assert_eq!(got, Err(LocalIntelligenceFailure::Timeout));
        assert!(
            took < Duration::from_secs(2),
            "the absolute deadline must win, not the ~1.6s trickle: took {took:?}"
        );
        let _ = server.join();
    }

    /// Control for the trickle test: a reply that finishes comfortably before the
    /// deadline is still accepted (the deadline is not falsely tripping early).
    #[test]
    fn a_real_peer_that_finishes_in_time_still_succeeds() {
        use std::net::{TcpListener, TcpStream as StdStream};
        use std::thread;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let Ok((mut peer, _)) = listener.accept() else {
                return;
            };
            thread::sleep(Duration::from_millis(50));
            let _ = peer.write_all(b"hello");
        });
        let mut client = StdStream::connect(addr).unwrap();
        let deadline = Instant::now() + Duration::from_millis(800);
        let got = read_bounded(&mut client, deadline, MAXIMUM_HTTP_RESPONSE_BYTES);
        assert_eq!(got, Ok(b"hello".to_vec()));
        let _ = server.join();
    }

    #[test]
    fn rejects_non_loopback() {
        assert!(LoopbackLocalProvider::new("8.8.8.8:80".parse().unwrap(), "x").is_err())
    }

    // ------------------------------------ capability contract (M0.15.9) ------

    /// `to_provider_failure` reproduces the exact `LocalProviderFailure` every
    /// one of these underlying conditions already produced before M0.15.9 (see
    /// the module documentation table). A future new variant added to
    /// `LocalIntelligenceFailure` without extending this match is a compile
    /// error, not a silently-wrong mapping.
    #[test]
    fn to_provider_failure_reproduces_the_pre_m0_15_9_mapping() {
        use LocalIntelligenceFailure as L;
        use LocalProviderFailure as P;
        assert_eq!(L::RuntimeUnavailable.to_provider_failure(), P::Unavailable);
        assert_eq!(L::Timeout.to_provider_failure(), P::Timeout);
        assert_eq!(L::WriteFailure.to_provider_failure(), P::ProviderError);
        assert_eq!(L::ResponseTooLarge.to_provider_failure(), P::ProviderError);
        assert_eq!(L::ModelRejected.to_provider_failure(), P::ProviderError);
        assert_eq!(
            L::MalformedResponse.to_provider_failure(),
            P::MalformedResponse
        );
        assert_eq!(L::Other.to_provider_failure(), P::ProviderError);
    }

    /// `status()` reports availability from a real, bounded loopback probe: true
    /// while something is listening, false once nothing is. Bounded means
    /// bounded: this must return quickly against a listener that never accepts.
    #[test]
    fn status_reports_availability_from_a_real_bounded_probe() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let provider = LoopbackLocalProvider::new(endpoint, "qwen-test").unwrap();

        let started = Instant::now();
        let status = provider.status();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "status() must be bounded"
        );
        assert!(status.available, "a real listener must report available");
        assert_eq!(status.model, "qwen-test");

        drop(listener);
        let status = provider.status();
        assert!(!status.available, "no listener must report unavailable");
        assert_eq!(
            status.model, "qwen-test",
            "model identity is reported either way"
        );
    }

    /// `warm()` does not block the caller, and it does send exactly the
    /// documented empty-prompt request (proving generation is never actually
    /// requested for a warm-up).
    #[test]
    fn warm_sends_an_empty_prompt_request_without_blocking_the_caller() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = socket.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
            let _ = socket.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 0\r\n\r\n");
        });

        let provider = LoopbackLocalProvider::new(endpoint, "qwen-test").unwrap();
        let started = Instant::now();
        provider.warm();
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "warm() must not block the calling thread"
        );

        let request = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(request.starts_with("POST /api/generate HTTP/1.0\r\n"));
        assert!(
            request.contains("\"prompt\":\"\""),
            "no real prompt is ever sent: {request}"
        );
        assert!(!request.contains("\"prompt\":\" "), "{request}");
    }

    /// `complete_detailed` distinguishes a real, successful completion with
    /// timing — the concrete new thing this capability adds beyond `complete`.
    #[test]
    fn complete_detailed_returns_timing_alongside_the_text() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf);
            std::thread::sleep(Duration::from_millis(30));
            let _ = socket.write_all(
                b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{\"response\":\"warm answer\"}",
            );
        });
        let provider = LoopbackLocalProvider::new(endpoint, "qwen-test").unwrap();
        let outcome = provider.complete_detailed("hello", 64).unwrap();
        assert_eq!(outcome.text, "warm answer");
        assert!(
            outcome.duration >= Duration::from_millis(30),
            "got {:?}",
            outcome.duration
        );
        assert!(
            outcome.duration < Duration::from_secs(5),
            "got {:?}",
            outcome.duration
        );
    }

    /// `complete_detailed` distinguishes a dead listener as `RuntimeUnavailable`
    /// (not the generic `Other`/`ModelRejected`), and `complete()` on the exact
    /// same input still maps it down to the pre-existing `Unavailable`.
    #[test]
    fn complete_detailed_distinguishes_runtime_unavailable_and_complete_still_maps_down() {
        let addr = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap()
            // dropped: nothing accepts on this port
        };
        let provider = LoopbackLocalProvider::new(addr, "qwen-test").unwrap();
        assert_eq!(
            provider.complete_detailed("hello", 64).unwrap_err(),
            LocalIntelligenceFailure::RuntimeUnavailable
        );
        assert_eq!(
            provider.complete("hello", 64).unwrap_err(),
            LocalProviderFailure::Unavailable
        );
    }

    /// `complete_detailed` distinguishes an oversized response as
    /// `ResponseTooLarge`, a case `complete()` alone could never tell apart
    /// from a malformed one — and `complete()` on the identical input still
    /// maps it down to the pre-existing `ProviderError`.
    #[test]
    fn complete_detailed_distinguishes_response_too_large_and_complete_still_maps_down() {
        for check in [true, false] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = listener.local_addr().unwrap();
            std::thread::spawn(move || {
                let Ok((mut socket, _)) = listener.accept() else {
                    return;
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf);
                let _ =
                    socket.write_all(b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n");
                let chunk = vec![b'x'; 64 * 1024];
                for _ in 0..((MAXIMUM_HTTP_RESPONSE_BYTES / chunk.len()) + 4) {
                    if socket.write_all(&chunk).is_err() {
                        return;
                    }
                }
            });
            let provider = LoopbackLocalProvider::new(endpoint, "qwen-test").unwrap();
            if check {
                assert_eq!(
                    provider.complete_detailed("hello", 64).unwrap_err(),
                    LocalIntelligenceFailure::ResponseTooLarge
                );
            } else {
                assert_eq!(
                    provider.complete("hello", 64).unwrap_err(),
                    LocalProviderFailure::ProviderError
                );
            }
        }
    }

    /// A request the runtime's own contract would refuse (an out-of-range
    /// output budget) is classified `ModelRejected`, distinct from a genuine
    /// transport failure, and never reaches the network.
    #[test]
    fn complete_detailed_classifies_an_invalid_budget_as_model_rejected_before_any_network() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            if listener.accept().is_ok() {
                let _ = tx.send(());
            }
        });
        let provider = LoopbackLocalProvider::new(endpoint, "qwen-test").unwrap();
        assert_eq!(
            provider.complete_detailed("hello", 0).unwrap_err(),
            LocalIntelligenceFailure::ModelRejected
        );
        assert_eq!(
            provider
                .complete_detailed("hello", MAXIMUM_COMPLETION_OUTPUT_TOKENS + 1)
                .unwrap_err(),
            LocalIntelligenceFailure::ModelRejected
        );
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "an invalid budget must never reach the network"
        );
    }
    /// Reused from the earlier protected work: a large (>4 KB) unicode envelope,
    /// close-delimited, is parsed through the Briefing `consult` path too.
    #[test]
    fn large_unicode_envelope_uses_close_delimited_http() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let expected = "Źródła zachowują niezmienną treść. ".repeat(150);
        let claim = expected.clone();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = socket.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
                if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                    let header = std::str::from_utf8(&request[..end]).unwrap();
                    assert!(header.starts_with("POST /api/generate HTTP/1.0\r\n"));
                    let length: usize = header
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let response = serde_json::json!({"claims":[{"text":claim,"class":"SupportedByEvidence","citations":["e1"]}]}).to_string();
            let envelope = serde_json::json!({"response":response}).to_string();
            assert!(envelope.len() > 4096);
            socket
                .write_all(b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n")
                .unwrap();
            socket.write_all(envelope.as_bytes()).unwrap();
        });
        let packet = BriefingPacket {
            id: "test".into(),
            workspace_id: "workspace".into(),
            objective: "Summarize evidence".into(),
            instructions: "Cite evidence".into(),
            template_version: "v1".into(),
            response_contract: "claims".into(),
            privacy_classification: "private".into(),
            assurance: "A1".into(),
            routing_constraints: "local".into(),
            evidence: vec![],
        };
        let provider = LoopbackLocalProvider::new(endpoint, "test-model").unwrap();
        let result = provider.consult(&packet, MAXIMUM_OUTPUT_TOKENS).unwrap();
        assert_eq!(result.claims[0].text, expected);
        server.join().unwrap();
    }
}

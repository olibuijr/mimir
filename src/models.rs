//! Calls to the embedding and reranking models (an OpenAI-style API such as llama.cpp's llama-server).
use anyhow::Result;
use serde_json::{json, Value};
use std::time::Duration;

pub struct Models {
    pub url: String,
    pub rerank_url: String,
    pub embed_model: String,
    pub rerank_model: String,
    agent: ureq::Agent,
    embed_timeout: Duration,
    rerank_timeout: Duration,
}

/// A failed call to a model server, classified so the API can tell an outage from a Mímir bug.
/// Messages name the call and the server (credentials in the URL removed), never request data.
#[derive(Debug)]
pub enum ModelError {
    /// The model server answered with an error status or a response Mímir cannot use (502).
    Bad(String),
    /// The model server could not be reached (503).
    Down(String),
    /// The model server did not answer in time (504).
    Timeout(String),
}

impl ModelError {
    /// The HTTP status the API answers with.
    pub fn status(&self) -> u16 {
        match self {
            ModelError::Bad(_) => 502,
            ModelError::Down(_) => 503,
            ModelError::Timeout(_) => 504,
        }
    }

    fn from_ureq(what: &str, url: &str, e: ureq::Error) -> Self {
        let url = redact(url);
        match e {
            ureq::Error::Status(code, r) => {
                let body = r.into_string().unwrap_or_default();
                ModelError::Bad(format!("{what}: model server {url} answered {code}: {}", clip(body.trim(), 200)))
            }
            ureq::Error::Transport(t) => {
                let io = std::error::Error::source(&t).and_then(|s| s.downcast_ref::<std::io::Error>()).map(std::io::Error::kind);
                if t.kind() == ureq::ErrorKind::Io && io == Some(std::io::ErrorKind::TimedOut) {
                    ModelError::Timeout(format!("{what}: model server {url} did not answer in time"))
                } else {
                    ModelError::Down(format!("{what}: model server {url} is unreachable ({})", t.kind()))
                }
            }
        }
    }

    /// Reading or decoding the response body failed.
    fn from_body(what: &str, url: &str, e: std::io::Error) -> Self {
        let url = redact(url);
        if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
            ModelError::Timeout(format!("{what}: model server {url} did not answer in time"))
        } else {
            ModelError::Bad(format!("{what}: model server {url} sent an unreadable response: {}", clip(&e.to_string(), 200)))
        }
    }

    fn bad_shape(what: &str, url: &str, r: &Value) -> Self {
        ModelError::Bad(format!("{what}: model server {} sent an unexpected response: {}", redact(url), clip(&r.to_string(), 200)))
    }
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (ModelError::Bad(m) | ModelError::Down(m) | ModelError::Timeout(m)) = self;
        f.write_str(m)
    }
}
impl std::error::Error for ModelError {}

/// A URL without the `user:password@` part.
pub fn redact(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let end = rest.find('/').unwrap_or(rest.len());
    match rest[..end].rfind('@') {
        Some(at) => format!("{scheme}://{}", &rest[at + 1..]),
        None => url.to_string(),
    }
}

/// POST JSON to a model server and read the JSON answer, with errors classified.
fn post_json(agent: &ureq::Agent, what: &str, url: &str, timeout: Duration, body: Value) -> Result<Value, ModelError> {
    let r = agent.post(url).timeout(timeout).send_json(body).map_err(|e| ModelError::from_ureq(what, url, e))?;
    r.into_json().map_err(|e| ModelError::from_body(what, url, e))
}

/// Instruction for Qwen3-reranker: makes the score a calibrated "is this skill needed?" judgement.
pub const RERANK_INSTRUCTION: &str = "Given a request a user sent to an AI coding assistant, judge whether this skill's instructions would help the assistant carry out the request";

impl Models {
    pub fn from_env() -> Self {
        let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let ms = |k: &str, d: u64| Duration::from_millis(std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d));
        Models {
            url: var("MIMIR_MODELS_URL", "http://127.0.0.1:8084"),
            // Embeddings and reranking can come from two servers (one llama-server per model).
            rerank_url: std::env::var("MIMIR_RERANK_URL").unwrap_or_else(|_| var("MIMIR_MODELS_URL", "http://127.0.0.1:8084")),
            embed_model: var("MIMIR_EMBED_MODEL", "embeddinggemma-2-Q8_0"),
            rerank_model: var("MIMIR_RERANK_MODEL", "qwen3-reranker-0.6b-q8_0"),
            agent: ureq::AgentBuilder::new().timeout_connect(Duration::from_millis(500)).build(),
            // The prompt hook gives up after a few seconds, so the models must answer faster than that.
            embed_timeout: ms("MIMIR_EMBED_TIMEOUT_MS", 2000),
            rerank_timeout: ms("MIMIR_RERANK_TIMEOUT_MS", 3000),
        }
    }

    /// One query embedding on the hook's tight timeout, never waiting longer than `left` (what
    /// remains of the request's budget).
    pub fn embed_within(&self, inputs: &[String], left: Duration) -> Result<Vec<Vec<f32>>> {
        self.embed_with(inputs, self.embed_timeout.min(left))
    }

    /// Many documents (skill upload, reembed): not latency critical, so a long timeout.
    pub fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_with(inputs, Duration::from_secs(60))
    }

    fn embed_with(&self, inputs: &[String], timeout: Duration) -> Result<Vec<Vec<f32>>> {
        let (what, url) = ("embedding request", format!("{}/v1/embeddings", self.url));
        let r = post_json(&self.agent, what, &url, timeout, json!({ "model": self.embed_model, "input": inputs }))?;
        let mut data = r["data"].as_array().filter(|d| d.len() == inputs.len()).ok_or_else(|| ModelError::bad_shape(what, &url, &r))?.clone();
        data.sort_by_key(|d| d["index"].as_u64().unwrap_or(0));
        data.iter()
            .map(|d| {
                let v = d["embedding"].as_array().filter(|v| !v.is_empty()).ok_or_else(|| ModelError::bad_shape(what, &url, &r))?;
                Ok(normalize(v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()))
            })
            .collect()
    }

    /// One score per document, in document order, never waiting longer than `left` (what remains
    /// of the request's budget).
    pub fn rerank_within(&self, query: &str, docs: &[String], left: Duration) -> Result<Vec<f32>> {
        if docs.is_empty() {
            return Ok(vec![]);
        }
        let mut body = json!({ "model": self.rerank_model, "query": query, "documents": docs });
        // MIMIR_RERANK_INSTRUCTION overrides the instruction; set it empty to send none (for A/B runs).
        let instruction = std::env::var("MIMIR_RERANK_INSTRUCTION").unwrap_or_else(|_| RERANK_INSTRUCTION.into());
        if !instruction.is_empty() {
            body["instruction"] = json!(instruction);
        }
        let (what, url) = ("rerank request", format!("{}/v1/rerank", self.rerank_url));
        let r = post_json(&self.agent, what, &url, self.rerank_timeout.min(left), body)?;
        let mut scores = vec![0.0; docs.len()];
        for x in r["results"].as_array().ok_or_else(|| ModelError::bad_shape(what, &url, &r))? {
            let i = x["index"].as_u64().unwrap_or(0) as usize;
            if i < scores.len() {
                scores[i] = x["relevance_score"].as_f64().unwrap_or(0.0) as f32;
            }
        }
        Ok(scores)
    }
}

impl Models {
    /// Health check of the embedding server: one real one-input /v1/embeddings call.
    pub fn probe_embed(&self) -> Result<()> {
        self.embed_with(&["ping".into()], PROBE_TIMEOUT).map(drop)
    }

    /// Health check of the rerank server: one real one-document /v1/rerank call. An embeddings-only
    /// server answers with an error here, so a MIMIR_RERANK_URL pointing at it fails.
    pub fn probe_rerank(&self) -> Result<()> {
        let (what, url) = ("rerank request", format!("{}/v1/rerank", self.rerank_url));
        let body = json!({ "model": self.rerank_model, "query": "ping", "documents": ["ping"] });
        let r = post_json(&self.agent, what, &url, PROBE_TIMEOUT, body)?;
        match r["results"].as_array() {
            Some(a) if a.len() == 1 && a[0]["relevance_score"].is_number() => Ok(()),
            _ => Err(ModelError::bad_shape(what, &url, &r).into()),
        }
    }
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// EmbeddingGemma prompt formats.
pub fn query_text(q: &str) -> String {
    format!("task: search result | query: {}", clip(q, 4000))
}
pub fn doc_text(name: &str, description: &str) -> String {
    format!("title: {name} | text: {description}")
}

/// Long prompts (pasted logs, diffs) drown the request: keep the first 700 and last 300 chars.
/// Returns the text to embed/rerank/log and the original length in chars.
pub fn clip_query(q: &str) -> (String, usize) {
    let n = q.chars().count();
    if n <= 1000 {
        return (q.to_string(), n);
    }
    let head = clip(q, 700);
    let tail_start = q.char_indices().nth(n - 300).map_or(q.len(), |(i, _)| i);
    (format!("{head} … {}", &q[tail_start..]), n)
}

pub fn clip(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
    v
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A one-thread HTTP server on 127.0.0.1 that answers every request with `status` and `body`
    /// after `delay`. Returns its base URL.
    pub(crate) fn mock(status: u16, body: &'static str, delay: Duration) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { continue };
                std::thread::spawn(move || {
                    // Read the head, then the body by Content-Length, so the client sees a clean exchange.
                    let mut buf = vec![];
                    let mut b = [0u8; 4096];
                    let need = loop {
                        let Ok(n) = s.read(&mut b) else { return };
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&b[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                            let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                            break i + 4 + len;
                        }
                    };
                    while buf.len() < need {
                        let Ok(n) = s.read(&mut b) else { return };
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&b[..n]);
                    }
                    std::thread::sleep(delay);
                    let _ = write!(
                        s,
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                });
            }
        });
        url
    }

    /// A URL nothing listens on.
    pub(crate) fn dead_url() -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    }

    pub(crate) fn models(url: &str, rerank_url: &str) -> Models {
        models_with(url, rerank_url, Duration::from_millis(300))
    }

    /// Test models whose own embed and rerank timeouts are both `timeout`.
    pub(crate) fn models_with(url: &str, rerank_url: &str, timeout: Duration) -> Models {
        Models {
            url: url.into(),
            rerank_url: rerank_url.into(),
            embed_model: "e".into(),
            rerank_model: "r".into(),
            agent: ureq::AgentBuilder::new().timeout_connect(Duration::from_millis(500)).build(),
            embed_timeout: timeout,
            rerank_timeout: timeout,
        }
    }

    fn kind(r: Result<Vec<Vec<f32>>>) -> u16 {
        r.unwrap_err().downcast_ref::<ModelError>().expect("a ModelError").status()
    }

    pub(crate) const EMBED_OK: &str = r#"{"data":[{"index":0,"embedding":[3.0,4.0]}]}"#;
    pub(crate) const RERANK_OK: &str = r#"{"results":[{"index":0,"relevance_score":0.75}]}"#;

    #[test]
    fn model_failures_are_classified() {
        let z = Duration::ZERO;
        let q = ["q".to_string()];
        assert_eq!(models(&mock(200, EMBED_OK, z), "").embed_within(&q, Duration::MAX).unwrap(), vec![vec![0.6, 0.8]]);
        assert_eq!(kind(models(&mock(500, r#"{"error":"boom"}"#, z), "").embed_within(&q, Duration::MAX)), 502);
        assert_eq!(kind(models(&mock(200, r#"{"data":[]}"#, z), "").embed_within(&q, Duration::MAX)), 502);
        assert_eq!(kind(models(&mock(200, "not json", z), "").embed_within(&q, Duration::MAX)), 502);
        assert_eq!(kind(models(&dead_url(), "").embed_within(&q, Duration::MAX)), 503);
        assert_eq!(kind(models(&mock(200, EMBED_OK, Duration::from_millis(1500)), "").embed_within(&q, Duration::MAX)), 504);
        let m = models("", &mock(501, r#"{"error":{"message":"This server does not support reranking"}}"#, z));
        let e = m.rerank_within("q", &["d".into()], Duration::MAX).unwrap_err();
        assert_eq!(e.downcast_ref::<ModelError>().unwrap().status(), 502);
        assert!(e.to_string().contains("501") && e.to_string().contains("does not support reranking"), "{e}");
        assert_eq!(models("", &mock(200, RERANK_OK, z)).rerank_within("q", &["d".into()], Duration::MAX).unwrap(), vec![0.75]);
    }

    #[test]
    fn probes_call_each_model() {
        let z = Duration::ZERO;
        let (embed, rerank) = (mock(200, EMBED_OK, z), mock(200, RERANK_OK, z));
        assert!(models(&embed, &rerank).probe_embed().is_ok());
        assert!(models(&embed, &rerank).probe_rerank().is_ok());
        // The rerank URL left pointing at the embeddings server: it does not answer /v1/rerank.
        let only_embed = mock(501, r#"{"error":{"message":"This server does not support reranking. Start it with --reranking"}}"#, z);
        assert!(models(&only_embed, &only_embed).probe_embed().is_err());
        assert!(models(&embed, &only_embed).probe_rerank().is_err());
        // A server that answers 200 with an embeddings body is no reranker either.
        assert!(models(&embed, &embed).probe_rerank().is_err());
        assert!(models(&rerank, &rerank).probe_embed().is_err());
        assert!(models(&dead_url(), &rerank).probe_embed().is_err());
    }

    #[test]
    fn urls_in_messages_lose_credentials() {
        assert_eq!(redact("http://u:secret@host:8084/v1/rerank"), "http://host:8084/v1/rerank");
        assert_eq!(redact("http://host:8084/a@b"), "http://host:8084/a@b");
        assert_eq!(redact("host"), "host");
        let url = dead_url().replace("http://", "http://u:secret@");
        let e = models(&url, "").embed_within(&["q".into()], Duration::MAX).unwrap_err();
        assert!(!e.to_string().contains("secret"), "{e}");
    }

    #[test]
    fn short_query_unchanged() {
        let q = "é".repeat(1000);
        assert_eq!(clip_query(&q), (q.clone(), 1000));
    }

    #[test]
    fn long_query_keeps_head_and_tail() {
        let q = format!("{}{}{}", "a".repeat(700), "ö".repeat(500), "z".repeat(300));
        let (c, n) = clip_query(&q);
        assert_eq!(n, 1500);
        assert_eq!(c.chars().count(), 700 + 3 + 300);
        assert!(c.starts_with(&"a".repeat(700)) && c.ends_with(&"z".repeat(300)));
        assert!(c.contains(" … "));
    }
}

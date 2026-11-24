//! Calls to the embedding and reranking models (an OpenAI-style API such as llama.cpp's llama-server).
use anyhow::{anyhow, Context, Result};
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

    /// One query embedding on the hook's tight timeout.
    pub fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_with(inputs, self.embed_timeout)
    }

    /// Many documents (skill upload, reembed): not latency critical, so a long timeout.
    pub fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_with(inputs, Duration::from_secs(60))
    }

    fn embed_with(&self, inputs: &[String], timeout: Duration) -> Result<Vec<Vec<f32>>> {
        let r: Value = self
            .agent
            .post(&format!("{}/v1/embeddings", self.url))
            .timeout(timeout)
            .send_json(json!({ "model": self.embed_model, "input": inputs }))
            .context("embedding request")?
            .into_json()?;
        let mut data = r["data"].as_array().ok_or_else(|| anyhow!("bad embeddings response: {r}"))?.clone();
        data.sort_by_key(|d| d["index"].as_u64().unwrap_or(0));
        data.iter()
            .map(|d| {
                let v = d["embedding"].as_array().ok_or_else(|| anyhow!("no embedding"))?;
                Ok(normalize(v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()))
            })
            .collect()
    }

    /// One score per document, in document order.
    pub fn rerank(&self, query: &str, docs: &[String]) -> Result<Vec<f32>> {
        if docs.is_empty() {
            return Ok(vec![]);
        }
        let mut body = json!({ "model": self.rerank_model, "query": query, "documents": docs });
        // MIMIR_RERANK_INSTRUCTION overrides the instruction; set it empty to send none (for A/B runs).
        let instruction = std::env::var("MIMIR_RERANK_INSTRUCTION").unwrap_or_else(|_| RERANK_INSTRUCTION.into());
        if !instruction.is_empty() {
            body["instruction"] = json!(instruction);
        }
        let r: Value = self
            .agent
            .post(&format!("{}/v1/rerank", self.rerank_url))
            .timeout(self.rerank_timeout)
            .send_json(body)
            .context("rerank request")?
            .into_json()?;
        let mut scores = vec![0.0; docs.len()];
        for x in r["results"].as_array().ok_or_else(|| anyhow!("bad rerank response: {r}"))? {
            let i = x["index"].as_u64().unwrap_or(0) as usize;
            if i < scores.len() {
                scores[i] = x["relevance_score"].as_f64().unwrap_or(0.0) as f32;
            }
        }
        Ok(scores)
    }
}

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
mod tests {
    use super::*;

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

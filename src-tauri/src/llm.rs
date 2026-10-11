//! Backend LLM client
//!
//! A small, blocking, single-prompt client for backend features that need the
//! technician's configured model (e.g. the startup optimizer). It reads the
//! same agent settings as the chat UI and interprets base URLs the same way:
//! for Ollama and custom providers the base URL is the OpenAI-compatible root
//! (e.g. `http://localhost:11434/v1`).
//!
//! Must be called from a blocking context (service threads, `spawn_blocking`).

use std::time::Duration;

use serde_json::{json, Value};

use crate::commands::settings::get_settings;
use crate::types::{AgentProvider, AgentSettings};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434/v1";

/// Resolved connection details for the configured provider.
pub struct LlmConfig {
    pub provider: AgentProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Whether a provider works without an API key.
pub fn provider_needs_key(provider: &AgentProvider) -> bool {
    !matches!(provider, AgentProvider::Ollama)
}

/// Resolve the configured provider, or `None` if it isn't usable.
pub fn config_from_settings(agent: &AgentSettings) -> Option<LlmConfig> {
    let keys = &agent.api_keys;
    let (base_url, key) = match agent.provider {
        AgentProvider::OpenAI => ("https://api.openai.com/v1".to_string(), keys.openai.clone()),
        AgentProvider::Anthropic => (
            "https://api.anthropic.com".to_string(),
            keys.anthropic.clone(),
        ),
        AgentProvider::Google => (
            "https://generativelanguage.googleapis.com".to_string(),
            keys.google.clone(),
        ),
        AgentProvider::Xai => ("https://api.x.ai/v1".to_string(), keys.xai.clone()),
        AgentProvider::DeepSeek => (
            "https://api.deepseek.com/v1".to_string(),
            keys.deepseek.clone(),
        ),
        AgentProvider::Groq => (
            "https://api.groq.com/openai/v1".to_string(),
            keys.groq.clone(),
        ),
        AgentProvider::Mistral => (
            "https://api.mistral.ai/v1".to_string(),
            keys.mistral.clone(),
        ),
        AgentProvider::OpenRouter => (
            "https://openrouter.ai/api/v1".to_string(),
            keys.openrouter.clone(),
        ),
        AgentProvider::Ollama => (
            agent
                .base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            Some("ollama".to_string()),
        ),
        AgentProvider::Custom => (
            agent.base_url.clone().filter(|u| !u.trim().is_empty())?,
            keys.custom.clone(),
        ),
    };

    let api_key = key.unwrap_or_default();
    if api_key.trim().is_empty()
        && provider_needs_key(&agent.provider)
        && agent.provider != AgentProvider::Custom
    {
        return None;
    }
    if agent.model.trim().is_empty() {
        return None;
    }

    Some(LlmConfig {
        provider: agent.provider.clone(),
        base_url: base_url.trim_end_matches('/').to_string(),
        api_key,
        model: agent.model.clone(),
    })
}

/// The configured provider, if one is usable.
pub fn configured() -> Option<LlmConfig> {
    config_from_settings(&get_settings().ok()?.agent)
}

/// Send one user prompt and return the model's text reply.
///
/// `json_reply` asks OpenAI-compatible providers for a JSON object response.
pub fn complete(config: &LlmConfig, prompt: &str, json_reply: bool) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let (request, extract): (_, fn(&Value) -> Option<&str>) = match config.provider {
        AgentProvider::Anthropic => (
            client
                .post(format!("{}/v1/messages", config.base_url))
                .header("x-api-key", &config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&json!({
                    "model": config.model,
                    "max_tokens": 2048,
                    "messages": [{ "role": "user", "content": prompt }],
                })),
            |b| b["content"][0]["text"].as_str(),
        ),
        AgentProvider::Google => (
            client
                .post(format!(
                    "{}/v1beta/models/{}:generateContent",
                    config.base_url, config.model
                ))
                .header("x-goog-api-key", &config.api_key)
                .json(&json!({ "contents": [{ "parts": [{ "text": prompt }] }] })),
            |b| b["candidates"][0]["content"]["parts"][0]["text"].as_str(),
        ),
        _ => {
            let mut body = json!({
                "model": config.model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": 0.1,
            });
            // Ollama and arbitrary custom servers don't all support response_format.
            if json_reply
                && !matches!(
                    config.provider,
                    AgentProvider::Ollama | AgentProvider::Custom
                )
            {
                body["response_format"] = json!({ "type": "json_object" });
            }
            let mut req = client
                .post(format!("{}/chat/completions", config.base_url))
                .json(&body);
            if !config.api_key.is_empty() {
                req = req.bearer_auth(&config.api_key);
            }
            (req, |b| b["choices"][0]["message"]["content"].as_str())
        }
    };

    let resp = request
        .send()
        .map_err(|e| format!("LLM request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "LLM API error {}: {}",
            status,
            truncate(&body, 500)
        ));
    }
    let body: Value = resp
        .json()
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;
    extract(&body)
        .map(str::to_owned)
        .ok_or_else(|| "LLM response contained no text".to_string())
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_works_without_key_and_uses_v1_base() {
        let agent = AgentSettings {
            provider: AgentProvider::Ollama,
            model: "llama3.2".into(),
            ..AgentSettings::default()
        };
        let cfg = config_from_settings(&agent).unwrap();
        assert_eq!(cfg.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn keyed_provider_without_key_is_unconfigured() {
        let agent = AgentSettings::default();
        assert!(config_from_settings(&agent).is_none());
    }
}

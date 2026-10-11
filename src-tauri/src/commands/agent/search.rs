//! Web search operations
//!
//! The provider and its credentials come from settings, so API keys never
//! travel through the webview and the agent and MCP server behave the same.

use std::time::Duration;

use serde_json::json;

use super::{get_settings, SearchResult};
use crate::types::SearchProvider;

const SEARCH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESULTS: usize = 5;

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(SEARCH_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn parse_results(data: &serde_json::Value) -> Vec<SearchResult> {
    data["results"]
        .as_array()
        .map(|results| {
            results
                .iter()
                .take(MAX_RESULTS)
                .map(|r| SearchResult {
                    title: r["title"].as_str().unwrap_or("").to_string(),
                    url: r["url"].as_str().unwrap_or("").to_string(),
                    snippet: r["content"].as_str().unwrap_or("").to_string(),
                    score: r["score"].as_f64(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Search the web using Tavily
async fn search_tavily(query: &str, api_key: &str) -> Result<Vec<SearchResult>, String> {
    let response = client()?
        .post("https://api.tavily.com/search")
        .json(&json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": false,
            "include_images": false,
            "max_results": MAX_RESULTS
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Tavily API error: {}", response.status()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(parse_results(&data))
}

/// Search the web using SearXNG
async fn search_searxng(query: &str, instance_url: &str) -> Result<Vec<SearchResult>, String> {
    let url = format!(
        "{}/search?q={}&format=json",
        instance_url.trim_end_matches('/'),
        urlencoding::encode(query)
    );

    let response = client()?
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("SearXNG error: {}", response.status()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(parse_results(&data))
}

/// Search the web with the provider configured in Settings -> AI Agent -> Web Search
pub(crate) async fn search_web(query: &str) -> Result<Vec<SearchResult>, String> {
    let agent = get_settings()?.agent;
    let not_configured = || {
        "No search provider configured. Set up Tavily or SearXNG in Settings -> AI Agent -> Web Search.".to_string()
    };

    match agent.search_provider {
        SearchProvider::Tavily => {
            let key = agent
                .tavily_api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(not_configured)?;
            search_tavily(query, &key).await
        }
        SearchProvider::Searxng => {
            let url = agent
                .searxng_url
                .filter(|u| !u.trim().is_empty())
                .ok_or_else(not_configured)?;
            search_searxng(query, &url).await
        }
        SearchProvider::None => Err(not_configured()),
    }
}

/// Search the web using the configured provider
#[tauri::command]
#[specta::specta]
pub async fn agent_search_web(query: String) -> Result<Vec<SearchResult>, String> {
    search_web(&query).await
}

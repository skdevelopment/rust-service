//! MCP Server Module
//!
//! Implements a Model Context Protocol server (JSON-RPC over HTTP) for remote
//! LLM control. Tools reuse the in-app agent's command functions.

mod server;
mod tools;

pub use server::apply_settings;

//! Per-run service context
//!
//! Every service gets a [`ServiceCtx`]: it collects logs and findings, emits
//! live `service-log` events in the one payload shape the UI listens for, runs
//! child processes so that cancelling the run actually stops them, and builds
//! the final [`ServiceResult`].

use std::io;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::types::{FindingSeverity, ServiceFinding, ServiceResult};

/// Set when the user cancels the current run; cleared when a run starts.
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Ask running services to stop (kills child processes started via [`ServiceCtx::output`]).
pub fn request_cancel() {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}

/// Reset the cancel flag at the start of a run.
pub fn reset_cancel() {
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

/// Whether the current run has been cancelled.
pub fn cancel_requested() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
}

/// Payload of the `service-log` event.
#[derive(serde::Serialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLogEvent {
    pub service_id: String,
    pub log: String,
    pub timestamp: String,
}

pub struct ServiceCtx {
    app: AppHandle,
    service_id: String,
    start: Instant,
    pub logs: Vec<String>,
    pub findings: Vec<ServiceFinding>,
}

impl ServiceCtx {
    pub fn new(app: &AppHandle, service_id: &str) -> Self {
        Self {
            app: app.clone(),
            service_id: service_id.to_string(),
            start: Instant::now(),
            logs: Vec::new(),
            findings: Vec::new(),
        }
    }

    /// Record a log line and stream it to the UI.
    pub fn log(&mut self, message: impl Into<String>) {
        let message = message.into();
        let _ = self.app.emit(
            "service-log",
            ServiceLogEvent {
                service_id: self.service_id.clone(),
                log: message.clone(),
                timestamp: Utc::now().to_rfc3339(),
            },
        );
        self.logs.push(message);
    }

    /// Add a finding.
    pub fn finding(
        &mut self,
        severity: FindingSeverity,
        title: impl Into<String>,
        description: impl Into<String>,
        recommendation: Option<String>,
        data: Option<Value>,
    ) {
        self.findings.push(ServiceFinding {
            severity,
            title: title.into(),
            description: description.into(),
            recommendation,
            data,
        });
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn is_cancelled(&self) -> bool {
        cancel_requested()
    }

    /// Build the final result.
    pub fn finish(self, success: bool, error: Option<String>) -> ServiceResult {
        ServiceResult {
            service_id: self.service_id.clone(),
            success,
            error,
            duration_ms: self.elapsed_ms(),
            findings: self.findings,
            logs: self.logs,
            agent_analysis: None,
            queue_item_id: None,
        }
    }

    /// Run `cmd` to completion like [`Command::output`], killing it if the run
    /// is cancelled. Returns `ErrorKind::Interrupted` when cancelled.
    pub fn output(&self, cmd: &mut Command) -> io::Result<Output> {
        self.run_child(cmd, None, None)
    }

    /// Like [`ServiceCtx::output`], writing `input` to the child's stdin first
    /// (for tools that ask a yes/no question).
    pub fn output_with_input(&self, cmd: &mut Command, input: &[u8]) -> io::Result<Output> {
        self.run_child(cmd, Some(input), None)
    }

    fn run_child(
        &self,
        cmd: &mut Command,
        input: Option<&[u8]>,
        timeout: Option<Duration>,
    ) -> io::Result<Output> {
        if cancel_requested() {
            return Err(cancelled_error());
        }
        let mut child = cmd
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            use std::io::Write;
            // Ignore broken pipes: the tool may exit without reading.
            let _ = stdin.write_all(input);
        }

        let stdout = child.stdout.take().map(drain);
        let stderr = child.stderr.take().map(drain);
        let deadline = timeout.map(|t| Instant::now() + t);

        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if cancel_requested() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(cancelled_error());
            }
            if deadline.is_some_and(|d| Instant::now() >= d) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "process timed out after {}s",
                        timeout.unwrap_or_default().as_secs()
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        };

        let join = |h: Option<std::thread::JoinHandle<Vec<u8>>>| {
            h.and_then(|h| h.join().ok()).unwrap_or_default()
        };
        Ok(Output {
            status,
            stdout: join(stdout),
            stderr: join(stderr),
        })
    }
}

fn cancelled_error() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "cancelled by user")
}

fn drain(mut pipe: impl io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

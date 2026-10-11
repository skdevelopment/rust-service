//! Service run state machine
//!
//! A run is started by [`run_services`], which validates the queue, records
//! the initial report and returns it immediately; the services then execute
//! on a blocking worker thread. Progress is published through the
//! `service-state-changed`, `service-progress` and `service-completed` events.
//!
//! Invariants:
//! - `is_running` stays true until the worker has actually finished, so a new
//!   run can never overlap a cancelled one that is still winding down;
//! - every exit path (success, cancel, error, panic) finalizes the report,
//!   saves it and clears `is_running`;
//! - pause/cancel wake a paused worker, and cancel also kills the child
//!   process of the service that is currently running.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use super::reports;
use crate::services;
use crate::types::{
    ServiceDefinition, ServiceQueueItem, ServiceReport, ServiceResult, ServiceRunState,
    ServiceRunStatus,
};

/// How often the parallel scheduler re-checks pause/cancel while waiting.
const PARALLEL_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Global state for the current (or most recent) run.
static SERVICE_STATE: Mutex<Option<ServiceRunState>> = Mutex::new(None);

/// Signalled on resume and cancel so a paused worker re-checks its state.
static PAUSE_CONDVAR: Condvar = Condvar::new();

fn state() -> MutexGuard<'static, Option<ServiceRunState>> {
    SERVICE_STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn snapshot() -> ServiceRunState {
    state().clone().unwrap_or_default()
}

fn emit_state(app: &AppHandle) {
    let _ = app.emit("service-state-changed", snapshot());
}

/// The report of the run in progress or last finished, if any.
pub(super) fn current_report() -> Option<ServiceReport> {
    state().as_ref().and_then(|s| s.current_report.clone())
}

/// Replace the in-memory copy of a report (after edits), if it is the current one.
pub(super) fn update_current_report(report: &ServiceReport) {
    if let Some(s) = state().as_mut() {
        if s.current_report.as_ref().is_some_and(|r| r.id == report.id) {
            s.current_report = Some(report.clone());
        }
    }
}

/// Get current service run state
#[tauri::command(async)]
#[specta::specta]
pub fn get_service_run_state() -> ServiceRunState {
    snapshot()
}

/// Start running services
///
/// Returns the initial report (with its id) as soon as the run has started.
#[tauri::command]
#[specta::specta]
pub async fn run_services(
    app: AppHandle,
    queue: Vec<ServiceQueueItem>,
    technician_name: Option<String>,
    customer_name: Option<String>,
    parallel: Option<bool>,
    agent_initiated: Option<bool>,
) -> Result<ServiceReport, String> {
    let parallel_mode = parallel.unwrap_or(false);

    let mut enabled_queue: Vec<_> = queue.iter().filter(|q| q.enabled).cloned().collect();
    enabled_queue.sort_by_key(|q| q.order);
    if enabled_queue.is_empty() {
        return Err("No services enabled in queue".to_string());
    }

    let definitions = services::get_all_definitions();
    validate_queue(&enabled_queue, &definitions)?;

    let report = ServiceReport {
        id: Uuid::new_v4().to_string(),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
        status: ServiceRunStatus::Running,
        total_duration_ms: None,
        queue,
        results: Vec::new(),
        current_service_index: Some(0),
        current_service_indices: vec![],
        parallel_mode,
        technician_name: technician_name.filter(|s| !s.trim().is_empty()),
        customer_name: customer_name.filter(|s| !s.trim().is_empty()),
        agent_initiated: agent_initiated.unwrap_or(false),
        agent_summary: None,
        health_score: None,
        error: None,
    };

    {
        let mut st = state();
        if st.as_ref().is_some_and(|s| s.is_running) {
            return Err("A service run is already in progress".to_string());
        }
        services::reset_cancel();
        *st = Some(ServiceRunState {
            is_running: true,
            is_paused: false,
            current_report: Some(report.clone()),
        });
    }
    emit_state(&app);

    let worker_app = app.clone();
    let worker_report = report.clone();
    tauri::async_runtime::spawn(async move {
        let start = Instant::now();
        let app = worker_app.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let mut report = worker_report;
            if parallel_mode {
                run_parallel(&worker_app, &enabled_queue, &definitions, &mut report);
            } else {
                run_sequential(&worker_app, &enabled_queue, &mut report);
            }
            report
        })
        .await;

        let report = match outcome {
            Ok(report) => finalize(report, start, None),
            Err(e) => {
                // The worker panicked; recover the last synced copy of the report.
                log::error!("Service run worker failed: {}", e);
                let report = current_report().unwrap_or_else(empty_report);
                finalize(report, start, Some(format!("Service run aborted: {}", e)))
            }
        };

        {
            let mut st = state();
            *st = Some(ServiceRunState {
                is_running: false,
                is_paused: false,
                current_report: Some(report.clone()),
            });
        }
        emit_state(&app);
        let _ = app.emit("service-completed", &report);
    });

    Ok(report)
}

/// Placeholder report for the (unexpected) case of a panicked worker with no state.
fn empty_report() -> ServiceReport {
    ServiceReport {
        id: Uuid::new_v4().to_string(),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
        status: ServiceRunStatus::Failed,
        total_duration_ms: None,
        queue: vec![],
        results: vec![],
        current_service_index: None,
        current_service_indices: vec![],
        parallel_mode: false,
        technician_name: None,
        customer_name: None,
        agent_initiated: false,
        agent_summary: None,
        health_score: None,
        error: None,
    }
}

/// Reject unknown services and dependencies that are queued later than their dependents.
fn validate_queue(
    enabled_queue: &[ServiceQueueItem],
    definitions: &[ServiceDefinition],
) -> Result<(), String> {
    let dep_map: HashMap<&str, &Vec<String>> = definitions
        .iter()
        .map(|d| (d.id.as_str(), &d.dependencies))
        .collect();

    let mut seen: HashSet<&str> = HashSet::new();
    for item in enabled_queue {
        let deps = dep_map
            .get(item.service_id.as_str())
            .ok_or_else(|| format!("Unknown service: {}", item.service_id))?;
        for dep in deps.iter() {
            let dep_in_queue = enabled_queue.iter().any(|q| q.service_id == *dep);
            if dep_in_queue && !seen.contains(dep.as_str()) {
                return Err(format!(
                    "Service '{}' depends on '{}' which must come earlier in the queue",
                    item.service_id, dep
                ));
            }
        }
        seen.insert(item.service_id.as_str());
    }
    Ok(())
}

/// Set the final status, duration and timestamps, then persist the report.
fn finalize(mut report: ServiceReport, start: Instant, error: Option<String>) -> ServiceReport {
    report.completed_at = Some(Utc::now().to_rfc3339());
    report.status = if services::cancel_requested() {
        ServiceRunStatus::Cancelled
    } else if error.is_some() || !report.results.iter().all(|r| r.success) {
        ServiceRunStatus::Failed
    } else {
        ServiceRunStatus::Completed
    };
    if error.is_some() {
        report.error = error;
    }
    report.total_duration_ms = Some(start.elapsed().as_millis() as u64);
    report.current_service_index = None;
    report.current_service_indices = vec![];

    if let Err(e) = reports::persist_completed_report(&report) {
        log::error!("Failed to save report {}: {}", report.id, e);
    }
    report
}

// =============================================================================
// Shared runner helpers
// =============================================================================

fn is_paused() -> bool {
    state().as_ref().is_some_and(|s| s.is_paused)
}

/// Block while the run is paused. Returns `true` if the run was cancelled.
fn wait_while_paused() -> bool {
    let mut st = state();
    while st.as_ref().is_some_and(|s| s.is_paused) && !services::cancel_requested() {
        st = PAUSE_CONDVAR.wait(st).unwrap_or_else(|e| e.into_inner());
    }
    services::cancel_requested()
}

fn update_indices(indices: &[usize]) {
    if let Some(r) = state().as_mut().and_then(|s| s.current_report.as_mut()) {
        r.current_service_indices = indices.to_vec();
        r.current_service_index = indices.first().copied();
    }
}

fn sync_results(results: &[ServiceResult]) {
    if let Some(r) = state().as_mut().and_then(|s| s.current_report.as_mut()) {
        r.results = results.to_vec();
    }
}

fn emit_progress(app: &AppHandle, index: usize, total_count: usize, service_id: &str) {
    let _ = app.emit(
        "service-progress",
        json!({
            "currentIndex": index,
            "totalCount": total_count,
            "serviceId": service_id
        }),
    );
}

fn record_metrics(queue_item: &ServiceQueueItem, duration_ms: u64) {
    let options_hash = Some(crate::commands::time_tracking::compute_options_hash(
        &queue_item.options,
    ));
    if let Err(e) = crate::commands::time_tracking::record_service_time(
        queue_item.service_id.clone(),
        duration_ms,
        None,
        options_hash,
    ) {
        log::warn!("Failed to record service time: {}", e);
    }
}

/// Run one queue item; unknown ids and panics become failed results.
fn run_one(app: &AppHandle, item: &ServiceQueueItem) -> ServiceResult {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        services::run_service(&item.service_id, &item.options, app)
    }));
    let mut result = match outcome {
        Ok(Some(result)) => result,
        Ok(None) => failed_result(item, format!("Unknown service: {}", item.service_id)),
        Err(_) => failed_result(item, "Service crashed unexpectedly".to_string()),
    };
    result.queue_item_id = Some(item.id.clone());
    if result.success {
        record_metrics(item, result.duration_ms);
    }
    result
}

fn failed_result(item: &ServiceQueueItem, error: String) -> ServiceResult {
    ServiceResult {
        service_id: item.service_id.clone(),
        success: false,
        error: Some(error),
        duration_ms: 0,
        findings: vec![],
        logs: vec![],
        agent_analysis: None,
        queue_item_id: Some(item.id.clone()),
    }
}

// =============================================================================
// Sequential runner
// =============================================================================

fn run_sequential(app: &AppHandle, queue: &[ServiceQueueItem], report: &mut ServiceReport) {
    let total = queue.len();
    for (index, item) in queue.iter().enumerate() {
        if services::cancel_requested() || wait_while_paused() {
            break;
        }

        update_indices(&[index]);
        emit_progress(app, index, total, &item.service_id);
        emit_state(app);

        report.results.push(run_one(app, item));
        sync_results(&report.results);
        emit_state(app);
    }
}

// =============================================================================
// Parallel runner (experimental)
// =============================================================================

/// Results of finished parallel workers (queue index, result), plus a condvar
/// the scheduler waits on.
type FinishedQueue = Arc<(Mutex<Vec<(usize, ServiceResult)>>, Condvar)>;

/// Run services concurrently. Services sharing an exclusive resource tag are
/// serialized; dependencies must complete first. Pausing stops new services
/// from starting; running ones finish.
fn run_parallel(
    app: &AppHandle,
    queue: &[ServiceQueueItem],
    definitions: &[ServiceDefinition],
    report: &mut ServiceReport,
) {
    let resources: HashMap<&str, &Vec<String>> = definitions
        .iter()
        .map(|d| (d.id.as_str(), &d.exclusive_resources))
        .collect();
    let deps: HashMap<&str, &Vec<String>> = definitions
        .iter()
        .map(|d| (d.id.as_str(), &d.dependencies))
        .collect();

    let total = queue.len();
    let finished: FinishedQueue = Arc::new((Mutex::new(Vec::new()), Condvar::new()));

    let mut started: HashSet<usize> = HashSet::new();
    let mut completed: HashSet<usize> = HashSet::new();
    let mut held: HashSet<String> = HashSet::new();
    let mut running: Vec<usize> = Vec::new();

    let empty = Vec::new();
    loop {
        // Collect finished services and release their resources.
        let done: Vec<(usize, ServiceResult)> = {
            let (lock, _) = &*finished;
            std::mem::take(&mut *lock.lock().unwrap_or_else(|e| e.into_inner()))
        };
        if !done.is_empty() {
            for (idx, result) in done {
                completed.insert(idx);
                running.retain(|&r| r != idx);
                for res in resources
                    .get(queue[idx].service_id.as_str())
                    .copied()
                    .unwrap_or(&empty)
                {
                    held.remove(res);
                }
                report.results.push(result);
            }
            sync_results(&report.results);
            update_indices(&running);
            emit_state(app);
        }

        if completed.len() == started.len()
            && (started.len() == total || services::cancel_requested())
        {
            break;
        }

        // Launch eligible services unless paused or cancelled.
        if !services::cancel_requested() && !is_paused() {
            for (index, item) in queue.iter().enumerate() {
                if started.contains(&index) {
                    continue;
                }
                let item_resources = resources
                    .get(item.service_id.as_str())
                    .copied()
                    .unwrap_or(&empty);
                if item_resources.iter().any(|r| held.contains(r)) {
                    continue;
                }
                let deps_done = deps
                    .get(item.service_id.as_str())
                    .copied()
                    .unwrap_or(&empty)
                    .iter()
                    .all(|dep| {
                        queue
                            .iter()
                            .position(|q| q.service_id == *dep)
                            .is_none_or(|i| completed.contains(&i))
                    });
                if !deps_done {
                    continue;
                }

                held.extend(item_resources.iter().cloned());
                started.insert(index);
                running.push(index);
                emit_progress(app, index, total, &item.service_id);
                update_indices(&running);
                emit_state(app);

                let (finished, app, item) = (Arc::clone(&finished), app.clone(), item.clone());
                std::thread::spawn(move || {
                    let result = run_one(&app, &item);
                    let (lock, cvar) = &*finished;
                    lock.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push((index, result));
                    cvar.notify_all();
                });
            }
        }

        if running.is_empty()
            && started.len() < total
            && !services::cancel_requested()
            && !is_paused()
        {
            // Nothing running and nothing could start: unsatisfiable queue.
            log::error!(
                "Parallel scheduler stalled; {} services could not start",
                total - started.len()
            );
            break;
        }

        // Wait for a worker to finish (or for pause/cancel to change).
        let (lock, cvar) = &*finished;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_empty() {
            let _ = cvar.wait_timeout(guard, PARALLEL_POLL_INTERVAL);
        }
    }
}

// =============================================================================
// Control commands
// =============================================================================

/// Cancel the current service run
///
/// Stops queued services from starting and kills the running service's child
/// process. The run finishes (and `service-completed` fires) once the worker exits.
#[tauri::command(async)]
#[specta::specta]
pub fn cancel_service_run(app: AppHandle) -> Result<(), String> {
    {
        let mut st = state();
        let Some(s) = st.as_mut().filter(|s| s.is_running) else {
            return Err("No service run in progress".to_string());
        };
        services::request_cancel();
        s.is_paused = false;
        if let Some(report) = s.current_report.as_mut() {
            report.status = ServiceRunStatus::Cancelled;
        }
        PAUSE_CONDVAR.notify_all();
    }
    emit_state(&app);
    Ok(())
}

/// Pause the current service run (takes effect between services)
#[tauri::command(async)]
#[specta::specta]
pub fn pause_service_run(app: AppHandle) -> Result<(), String> {
    {
        let mut st = state();
        let Some(s) = st
            .as_mut()
            .filter(|s| s.is_running && !s.is_paused && !services::cancel_requested())
        else {
            return Err("No active service run to pause".to_string());
        };
        s.is_paused = true;
        if let Some(report) = s.current_report.as_mut() {
            report.status = ServiceRunStatus::Paused;
        }
    }
    emit_state(&app);
    Ok(())
}

/// Resume a paused service run
#[tauri::command(async)]
#[specta::specta]
pub fn resume_service_run(app: AppHandle) -> Result<(), String> {
    {
        let mut st = state();
        let Some(s) = st.as_mut().filter(|s| s.is_running && s.is_paused) else {
            return Err("No paused service run to resume".to_string());
        };
        s.is_paused = false;
        if let Some(report) = s.current_report.as_mut() {
            report.status = ServiceRunStatus::Running;
        }
        PAUSE_CONDVAR.notify_all();
    }
    emit_state(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(id: &str, deps: &[&str]) -> ServiceDefinition {
        ServiceDefinition {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            category: String::new(),
            estimated_duration_secs: 1,
            required_programs: vec![],
            options: vec![],
            icon: String::new(),
            exclusive_resources: vec![],
            dependencies: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn item(id: &str) -> ServiceQueueItem {
        ServiceQueueItem {
            id: format!("{}-item", id),
            service_id: id.into(),
            enabled: true,
            order: 0,
            options: json!({}),
        }
    }

    #[test]
    fn rejects_unknown_and_misordered_services() {
        let defs = vec![def("dism", &[]), def("sfc", &["dism"])];
        assert!(validate_queue(&[item("dism"), item("sfc")], &defs).is_ok());
        assert!(validate_queue(&[item("sfc")], &defs).is_ok());
        assert!(validate_queue(&[item("sfc"), item("dism")], &defs).is_err());
        assert!(validate_queue(&[item("nope")], &defs).is_err());
    }
}

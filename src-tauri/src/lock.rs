//! Mutex helpers that recover from poison instead of panicking.
//!
//! A poisoned mutex means a previous holder panicked. Recovering the inner
//! guard keeps production paths from aborting on `unwrap()`.

use std::sync::{Condvar, Mutex, MutexGuard};

/// Lock `mutex`, recovering the guard if a previous holder panicked.
pub fn lock_mutex<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Wait on `condvar`, recovering the guard if the mutex is poisoned.
pub fn wait_condvar<'a, T>(condvar: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    condvar
        .wait(guard)
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

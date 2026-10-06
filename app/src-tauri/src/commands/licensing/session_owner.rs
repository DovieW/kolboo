use crate::commands::{CommandError, CommandResult};
use std::sync::Mutex;

/// Serializes commits, not browser/network waits. Logout and cancellation can
/// invalidate an in-flight login immediately; refreshes have a separate queue.
pub(super) struct SessionOwner(Mutex<u64>);

impl SessionOwner {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(0))
    }

    pub(super) fn begin(&self) -> u64 {
        let mut epoch = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch = epoch.wrapping_add(1);
        *epoch
    }

    pub(super) fn current(&self) -> u64 {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn commit<T>(
        &self,
        ticket: u64,
        apply: impl FnOnce() -> CommandResult<T>,
    ) -> CommandResult<T> {
        let epoch = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *epoch != ticket {
            return Err(CommandError::new(
                "This sign-in operation was cancelled or replaced.",
                "auth",
            )
            .with_code("auth_operation_superseded"));
        }
        apply()
    }

    pub(super) fn invalidate_and_commit<T>(
        &self,
        apply: impl FnOnce() -> CommandResult<T>,
    ) -> CommandResult<T> {
        let mut epoch = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch = epoch.wrapping_add(1);
        apply()
    }
}

#[cfg(test)]
#[path = "tests/session_owner.rs"]
mod tests;

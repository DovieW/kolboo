//! Windows UI Automation (UIA) integration.
//
// This module will host UIA-focused context capture, insertion planning, and
// safety helpers for Windows-only reliability improvements.

pub mod app_identity;
pub mod capability_memory;
pub mod client;
pub mod com;
pub mod context;
pub mod insert;
pub mod insert_plan;
pub mod safety;
pub mod snapshot;
pub mod target_match;
pub mod types;
pub mod verify;

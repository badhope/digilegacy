//! Agent runtime boundary (AGT-01..04).
//!
//! The Agent may only act through Tool Registry / CommandBus. A write always
//! creates an Approval; a Run is auditable. M0 fixes the state machine terms.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Queued,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

pub const MODULE_FAMILY: &str = "AGT";
pub const STATUS: &str = "planned";

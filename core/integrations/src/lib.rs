//! Integration ports (INT-01..07).
//!
//! Integrations depend on ports, never on domain repositories. M5 wires the
//! real providers; M0 only fixes the kinds and status vocabulary.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Llm,
    Policy,
    Smtp,
    Imap,
    Heartbeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Unconfigured,
    Ok,
    Degraded,
    Error,
    Disabled,
}

pub const MODULE_FAMILY: &str = "INT";
pub const STATUS: &str = "planned";

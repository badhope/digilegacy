//! Store boundary (STO-01..04).
//!
//! M0 contains no SQL. M1 adds the SQLite/SQLCipher connection, schema v9 and
//! migrations. Repositories stay private to this crate; domains depend on ports.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreState {
    NotReady,
    Ready,
    Poisoned,
}

pub const MODULE_FAMILY: &str = "STO";
pub const STATUS: &str = "planned";

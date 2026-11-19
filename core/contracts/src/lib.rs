//! Contract descriptors generated from `contracts/`.
//!
//! This crate is intentionally dependency-free so every entry point can share it.
//! The generated files are checked in; `tools/generate.mjs --check` fails on drift.

mod errors;
mod generated;

pub use errors::{find as find_error, ErrorSpec, ERRORS};
pub use generated::{find as find_command, names, CommandSpec, COMMANDS};

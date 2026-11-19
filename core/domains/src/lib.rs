//! Domain services (DOM-01..07).
//!
//! M0 only declares the domain map. Each domain gets its own module in M2 and
//! may not import another domain's repository; cross-domain work goes through
//! application services.

#![forbid(unsafe_code)]

pub mod approvals;
pub mod assets;
pub mod audit;
pub mod discovery;
pub mod overview;
pub mod plan;
pub mod policy;
pub mod runs;
pub mod usage;

pub use assets::DomainError;

pub const DOMAINS: &[(&str, &str)] = &[
    ("DOM-01", "assets"),
    ("DOM-02", "policy"),
    ("DOM-03", "plan"),
    ("DOM-04", "trigger"),
    ("DOM-05", "deliver"),
    ("DOM-06", "review"),
    ("DOM-07", "drill"),
];

pub const STATUS: &str = "m2-assets-plan-policy";

//! Micro-baseline for the in-process command bus.
//! Run: cargo run -q -p digilegacy-foundation --release --example bus_bench
use digilegacy_foundation::{CommandBus, PlatformError};
use serde_json::{json, Value};
use std::hint::black_box;
use std::time::Instant;

fn echo(args: Value) -> Result<Value, PlatformError> {
    Ok(args)
}

fn main() {
    let mut bus = CommandBus::new();
    bus.register("app.overview", echo).expect("known command");
    let args = json!({"probe": true});
    let iterations = 1_000_000u32;

    // Warm the call path once, then measure only dispatch.
    black_box(bus.dispatch("app.overview", args.clone()));
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(bus.dispatch("app.overview", args.clone()));
    }
    let elapsed = started.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
    println!(
        "bus_bench: {} dispatches in {:?} ({:.1} ns/op, {:.0} ops/s)",
        iterations,
        elapsed,
        ns_per_op,
        1_000_000_000.0 / ns_per_op
    );
}

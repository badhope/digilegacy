//! End-to-end probe for the DNS-rebinding drill.
//!
//! `pin` resolves once, signals the harness, waits for the harness to change the
//! DNS answer, then connects only to the addresses returned by that one
//! resolution. `block` resolves after the answer has changed and must reject the
//! private address.
//!
//! This is a test binary, not a product entry point.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use digilegacy_security::resolve_tcp_host;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let host = args.get(2).map(String::as_str).unwrap_or("");
    let port: u16 = args
        .get(3)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    match mode {
        "pin" => pin(host, port, args.get(4), args.get(5)),
        "block" => block(host, port),
        _ => {
            eprintln!("usage: dns_rebind_probe pin <host> <port> <marker> <sleep-ms> | block <host> <port>");
            ExitCode::from(64)
        }
    }
}

fn pin(host: &str, port: u16, marker: Option<&String>, sleep_ms: Option<&String>) -> ExitCode {
    let addrs = match resolve_tcp_host(host, port) {
        Ok(addrs) => addrs,
        Err(error) => {
            println!("resolved=false error={error}");
            return ExitCode::FAILURE;
        }
    };
    println!("resolved=true addrs={addrs:?}");

    if let Some(path) = marker {
        if let Err(error) = fs::write(path, b"resolved") {
            println!("marker_error={error}");
            return ExitCode::FAILURE;
        }
    }
    let sleep_ms = sleep_ms
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(250);
    thread::sleep(Duration::from_millis(sleep_ms));

    let mut stream = match TcpStream::connect(&addrs[..]) {
        Ok(stream) => stream,
        Err(error) => {
            println!("connected=false error={error}");
            return ExitCode::FAILURE;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if let Err(error) = stream.write_all(b"ping") {
        println!("connected=false write_error={error}");
        return ExitCode::FAILURE;
    }
    let mut reply = [0_u8; 4];
    if let Err(error) = stream.read_exact(&mut reply) {
        println!("connected=false read_error={error}");
        return ExitCode::FAILURE;
    }
    println!("connected=true reply={}", String::from_utf8_lossy(&reply));
    ExitCode::SUCCESS
}

fn block(host: &str, port: u16) -> ExitCode {
    match resolve_tcp_host(host, port) {
        Ok(addrs) => {
            println!("blocked=false addrs={addrs:?}");
            ExitCode::FAILURE
        }
        Err(error) => {
            println!("blocked=true error={error}");
            ExitCode::SUCCESS
        }
    }
}

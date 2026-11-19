//! Optional desktop shell boundary.
//!
//! The product is a platform; the desktop app is only a packaging shell that
//! loads the same Web console. M0 reserves the binary so the workspace has four
//! entry points (server/cli/mcp/desktop); the Tauri window, tray and updater are
//! wired in M3/M5. Do not add business logic here.

fn main() {
    println!(
        "{}",
        serde_json::json!({
            "entry": "desktop-shell",
            "mode": "m0",
            "status": "shell-reserved",
            "note": "Loads the same Web console; window/tray/updater land in M3/M5."
        })
    );
}

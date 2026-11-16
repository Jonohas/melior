//! Workspace tooling. Run with `cargo xtask <task>`.
//!
//! Tasks:
//!
//! - `layers`: checks the dependency layering rules in [`layers`].

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod layers;

use std::process::{Command, ExitCode};

fn run_layers() -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    let (crates, edges) = layers::from_metadata(&meta);
    let found = layers::violations(&crates, &edges);
    if found.is_empty() {
        println!("layers: {} crates, {} edges, no violations", crates.len(), edges.len());
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let result = match task.as_deref() {
        Some("layers") => run_layers(),
        _ => Err("usage: cargo xtask layers".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

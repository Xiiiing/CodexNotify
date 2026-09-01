use codex_notify_core::{init_logging, process_hook_input, AppPaths};
use std::io::{Read, Write};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::discover()?;
    init_logging(&paths, "hook")?;
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    process_hook_input(&input).map_err(Into::into)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Codex notification failed: {error}");
    }
    let _ = std::io::stdout().write_all(b"{}\n");
}

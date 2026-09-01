fn main() {
    let target = std::env::var("TARGET").expect("TARGET is set by Cargo");
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let hook = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("binaries")
        .join(format!("codex-notify-hook-{target}{extension}"));
    println!("cargo:rerun-if-changed={}", hook.display());
    println!(
        "cargo:rustc-env=CODEX_NOTIFY_EMBEDDED_HOOK={}",
        hook.display()
    );
    tauri_build::build()
}

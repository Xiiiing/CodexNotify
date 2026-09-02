use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn single_binary_obeys_hook_protocol() {
    let data = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_codex-notify"))
        .arg("--codex-notify-hook")
        .env("CODEX_NOTIFY_DATA_DIR", data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"not-json").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "{}\n");
}

#[test]
fn init_writes_non_secret_settings() {
    let data = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-notify"))
        .args(["init", "--server", "https://bark.example.test"])
        .env("CODEX_NOTIFY_DATA_DIR", data.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let settings = std::fs::read_to_string(data.path().join("config/settings.json")).unwrap();
    assert!(settings.contains("https://bark.example.test"));
    assert!(!settings.contains("CODEX_NOTIFY_BARK_KEY"));
}

#[test]
fn installs_itself_as_the_codex_hook() {
    let codex_home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-notify"))
        .args(["hook", "install"])
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_NOTIFY_DATA_DIR", data.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let hooks = std::fs::read_to_string(codex_home.path().join("hooks.json")).unwrap();
    assert!(hooks.contains("--codex-notify-hook"));
    assert!(hooks.contains("PermissionRequest"));
    assert!(hooks.contains("PreToolUse"));
    assert!(hooks.contains("request_user_input"));
    assert!(hooks.contains("Stop"));

    let output = Command::new(env!("CARGO_BIN_EXE_codex-notify"))
        .args(["hook", "status"])
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_NOTIFY_DATA_DIR", data.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["installed"], true);
    assert_eq!(status["pathCurrent"], true);
}

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn invalid_json_still_returns_success_object() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_codex-notify-hook"))
        .env("CODEX_NOTIFY_DATA_DIR", temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"not-json")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "{}\n");
}

#[test]
fn diagnostic_health_does_not_mask_real_codex_activity() {
    let temp = tempfile::tempdir().unwrap();
    let run = |turn: &str, diagnostic: bool| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_codex-notify-hook"))
            .env("CODEX_NOTIFY_DATA_DIR", temp.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = serde_json::json!({
            "hook_event_name": "Stop",
            "session_id": "test",
            "turn_id": turn,
            "cwd": temp.path(),
            "last_assistant_message": "done",
            "diagnostic": diagnostic
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
    };

    run("diagnostic", true);
    assert!(temp.path().join("data/hook-diagnostic.json").exists());
    assert!(!temp.path().join("data/hook-health.json").exists());

    run("real", false);
    let health = std::fs::read_to_string(temp.path().join("data/hook-health.json")).unwrap();
    assert!(health.contains("\"turnId\": \"real\""));
}

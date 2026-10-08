use std::{fs, process::Command};

#[test]
fn headless_demo_is_runnable_and_labels_its_scope() {
    let result = Command::new(env!("CARGO_BIN_EXE_rustario64"))
        .arg("demo")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("144 Hz presentation schedule: 300 ticks"));
    assert!(stdout.contains("not Mario physics or original ROM integration"));
}

#[test]
fn invalid_commands_and_missing_rom_return_failure() {
    for args in [vec!["play"], vec!["inspect-rom", "no-such-private-rom.z64"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_rustario64"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
    }
}

#[test]
fn trace_cli_passes_equal_traces_and_fails_on_first_drift() {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("cli-traces-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let reference = dir.join("reference.trace.json");
    let candidate = dir.join("candidate.trace.json");
    let trace = rustario64::diagnostics::counter_replay(30, Default::default()).unwrap();
    fs::write(&reference, serde_json::to_vec(&trace).unwrap()).unwrap();
    fs::write(&candidate, serde_json::to_vec(&trace).unwrap()).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_rustario64"))
            .arg("compare-traces")
            .arg(&reference)
            .arg(&candidate)
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    let mut drifted = trace;
    drifted.frames[8].state.action = 1;
    fs::write(&candidate, serde_json::to_vec(&drifted).unwrap()).unwrap();
    let result = run();
    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("tick 9: frame.state.action")
    );
    fs::remove_dir_all(&dir).unwrap();
}

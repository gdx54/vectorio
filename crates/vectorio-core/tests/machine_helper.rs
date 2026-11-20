//! `vectorio-machine`, la sesión que corre en la Raspberry Pi: sin placa, avisa y termina
//! aunque el servicio mantenga abierta su entrada (si no, la placa quedaría ocupada).

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn without_a_board_it_reports_and_exits_with_its_input_still_open() {
    let dir = std::env::temp_dir().join(format!("vectorio-machine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let settings = dir.join("settings.json");
    std::fs::write(&settings, "{}").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vectorio-machine"))
        .args(["--port", dir.join("no-board").to_str().unwrap(), "--settings", settings.to_str().unwrap()])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap();
    let _input = child.stdin.take();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let note: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(note.get("Lost").is_some(), "{line}");
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(5) { let _ = child.kill(); panic!("sigue abierta"); }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bad_arguments_are_refused() {
    let status = Command::new(env!("CARGO_BIN_EXE_vectorio-machine")).stdin(Stdio::null()).stderr(Stdio::null()).status().unwrap();
    assert_eq!(status.code(), Some(2));
}

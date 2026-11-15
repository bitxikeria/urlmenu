use std::io::{IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

/// Starts the browser in its own process group and returns without waiting.
pub fn open(argv: &[String], dry_run: bool) -> Result<(), String> {
    if dry_run {
        println!("{}", serde_json::json!({ "action": "open", "argv": argv }));
        return Ok(());
    }
    let (program, args) = argv.split_first().ok_or("empty command")?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
        .map_err(|e| format!("cannot run {program:?}: {e}"))
}

pub fn copy(text: &str, dry_run: bool) -> Result<(), String> {
    if dry_run {
        println!("{}", serde_json::json!({ "action": "copy", "text": text }));
        return Ok(());
    }
    let argv: &[&str] = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        &["wl-copy"]
    } else {
        &["xclip", "-selection", "clipboard"]
    };
    let mut child = Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", argv[0]))?;
    child
        .stdin
        .take()
        .ok_or("clipboard stdin unavailable")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    child.wait().map(drop).map_err(|e| e.to_string())
}

/// Reports an error on stderr, and as a desktop notification when no terminal sees it.
pub fn report(message: &str) {
    eprintln!("urlmenu: {message}");
    if !std::io::stderr().is_terminal() {
        let _ = Command::new("notify-send")
            .args(["urlmenu", message])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

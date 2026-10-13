use std::{
    ffi::OsStr,
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn run(program: &OsStr, args: &[&OsStr], timeout: Duration) -> Result<String, String> {
    // Files avoid blocking on full stdout/stderr pipes while enforcing a deadline.
    let mut output = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut errors = tempfile::tempfile().map_err(|e| e.to_string())?;
    let stdout = output.try_clone().map_err(|e| e.to_string())?;
    let stderr = errors.try_clone().map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("Cannot start {}: {e}", program.to_string_lossy()))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            other => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match other {
                    Err(e) => format!("Cannot wait for {}: {e}", program.to_string_lossy()),
                    _ => {
                        format!("{} timed out. Try saving again.", program.to_string_lossy())
                    }
                });
            }
        }
    };
    let read = |file: &mut std::fs::File| -> Result<String, String> {
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    let text = read(&mut output)?;
    if !status.success() {
        let errors = read(&mut errors)?;
        let detail = [errors.trim(), text.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("{} failed: {detail}", program.to_string_lossy()));
    }
    Ok(text)
}

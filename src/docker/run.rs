use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::types::{CancelToken, ContainerCommand, ContainerResult, OutputLine};

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Spawn a container and return a channel that streams its output.
///
/// The caller receives [`OutputLine::Stdout`]/[`Stderr`] as they arrive,
/// followed by exactly one [`OutputLine::Done`] carrying the final result.
pub fn spawn(cmd: ContainerCommand, cancel: CancelToken) -> Result<Receiver<OutputLine>> {
    let mut child = Command::new("docker")
        .args(&cmd.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn docker process")?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        orchestrate(child, stdout, stderr, tx, cancel, cmd.timeout, cmd.log_path);
    });

    Ok(rx)
}

fn orchestrate(
    mut child: std::process::Child,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
    tx: Sender<OutputLine>,
    cancel: CancelToken,
    timeout: Duration,
    log_path: Option<std::path::PathBuf>,
) {
    // Accumulates all output for the final log / log_path write.
    let log_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));

    // --- reader threads ---------------------------------------------------
    let tx_out = tx.clone();
    let buf_out = log_buf.clone();
    let stdout_handle = std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if let Ok(mut buf) = buf_out.lock() {
                        buf.push_str(&l);
                        buf.push('\n');
                    }
                    // Receiver may be dropped — ignore send errors.
                    let _ = tx_out.send(OutputLine::Stdout(l));
                }
                Err(_) => break,
            }
        }
    });

    let tx_err = tx.clone();
    let buf_err = log_buf.clone();
    let stderr_handle = std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if let Ok(mut buf) = buf_err.lock() {
                        buf.push_str(&l);
                        buf.push('\n');
                    }
                    let _ = tx_err.send(OutputLine::Stderr(l));
                }
                Err(_) => break,
            }
        }
    });

    // --- poll loop ---------------------------------------------------------
    let start = Instant::now();
    let mut cancelled = false;
    let mut timed_out = false;

    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }

        if cancel.is_cancelled() {
            cancelled = true;
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }

        if start.elapsed() > timeout {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }

        std::thread::sleep(POLL_INTERVAL);
    };

    // --- finalize ----------------------------------------------------------
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let exit_code = exit_status.and_then(|s| s.code());
    let success = exit_code == Some(0);
    let log = log_buf.lock().map(|b| b.clone()).unwrap_or_default();

    if let Some(path) = log_path {
        let _ = std::fs::write(&path, &log);
    }

    let _ = tx.send(OutputLine::Done(ContainerResult {
        success,
        exit_code,
        log,
        cancelled,
        timed_out,
    }));
}

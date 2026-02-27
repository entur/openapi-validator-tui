use std::process::Command;

use anyhow::{Context, Result, bail};

/// Verify that the Docker daemon is reachable.
pub fn ensure_available() -> Result<()> {
    let status = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("failed to invoke `docker` — is it installed and on PATH?")?;

    if !status.success() {
        bail!("docker daemon is not running (exit {})", status);
    }
    Ok(())
}

/// Returns `["--user", "uid:gid"]` on Unix so containers write files
/// as the invoking user. Empty on other platforms.
pub fn user_args() -> Vec<String> {
    #[cfg(unix)]
    {
        // SAFETY: geteuid() and getegid() are simple POSIX getters that always succeed and have no side effects.
        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };
        vec!["--user".into(), format!("{uid}:{gid}")]
    }

    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_available_does_not_panic() {
        // We only assert it doesn't panic; CI may or may not have Docker.
        let _ = ensure_available();
    }

    #[cfg(unix)]
    #[test]
    fn user_args_returns_pair() {
        let args = user_args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--user");
        assert!(args[1].contains(':'));
    }
}

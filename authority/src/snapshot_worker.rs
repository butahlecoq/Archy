//! A separate process contains blocking protected-store reads. The parent owns
//! its deadline and kills and reaps the child before releasing review capacity.
use crate::protected_project::ProtectedProject;
use crate::{Store, hex_digest};
use std::io::{self, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_REPLY: u64 = 1024 * 1024 + 4096;

pub fn child_main(
    state: &Path,
    owner: u32,
    digest: &str,
    source_digest: &str,
    identity_digest: &str,
) -> io::Result<()> {
    if unsafe { libc::geteuid() } != owner || owner == 0 {
        return Err(io::Error::from(ErrorKind::PermissionDenied));
    }
    let store = Store::open(state, Some(owner))?;
    let project = ProtectedProject::open(&state.join("project/project.json"), owner)?;
    if project.source_sha256() != source_digest || project.identity_sha256() != identity_digest {
        return Err(io::Error::from(ErrorKind::PermissionDenied));
    }
    let proposal = store
        .read(digest)
        .map_err(|_| io::Error::from(ErrorKind::PermissionDenied))?;
    let snapshot = project.snapshot(&proposal)?;
    let text = String::from_utf8(snapshot.bytes().to_vec()).map_err(io::Error::other)?;
    let reply = serde_json::to_vec(&(text, snapshot.digest()))?;
    if reply.len() as u64 > MAX_REPLY {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    io::stdout().write_all(&reply)
}

pub fn run(
    executable: &Path,
    project: &ProtectedProject,
    owner: u32,
    digest: &str,
    deadline: Instant,
) -> io::Result<(String, String)> {
    let child = Command::new(executable)
        .arg("--snapshot-worker")
        .arg(project.state_dir())
        .arg(owner.to_string())
        .arg(digest)
        .arg(project.source_sha256())
        .arg(project.identity_sha256())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let bytes = collect(child, deadline)?;
    let (snapshot, reported): (String, String) = serde_json::from_slice(&bytes)?;
    if hex_digest(snapshot.as_bytes()) != reported {
        return Err(io::Error::from(ErrorKind::InvalidData));
    }
    Ok((snapshot, reported))
}

fn collect(mut child: std::process::Child, deadline: Instant) -> io::Result<Vec<u8>> {
    let stdout = child.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(MAX_REPLY + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_REPLY {
            return Err(io::Error::from(ErrorKind::InvalidData));
        }
        Ok::<_, io::Error>(bytes)
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                kill_and_reap(&mut child);
                let _ = reader.join();
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            kill_and_reap(&mut child);
            let _ = reader.join();
            return Err(io::Error::from(ErrorKind::TimedOut));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let bytes = reader
        .join()
        .map_err(|_| io::Error::from(ErrorKind::Other))??;
    if !status.success() || Instant::now() >= deadline {
        return Err(io::Error::from(ErrorKind::PermissionDenied));
    }
    Ok(bytes)
}

fn kill_and_reap(child: &mut std::process::Child) {
    let _ = child.kill();
    // A kernel-stuck child may still hold the project lock. Fail the service
    // closed rather than release capacity while it retains protected access.
    let reap_deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        if Instant::now() >= reap_deadline {
            std::process::abort();
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuck_worker_is_killed_and_reaped_at_deadline() {
        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let started = Instant::now();
        let result = collect(child, started + Duration::from_millis(50));
        assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
        let gone = unsafe { libc::kill(pid as i32, 0) };
        assert_eq!(gone, -1, "child must be reaped");
    }
}

use archy_core::error::ErrorCode;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{
    Pid, PidfdFlags, Signal, WaitId, WaitIdOptions, kill_process_group, pidfd_open,
    pidfd_send_signal, waitid,
};
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Command;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Exited(i32),
    Signaled(i32),
    TimedOut,
    Cancelled,
    OutputOverflow,
    LaunchUnavailable,
    IoFailure,
}

impl Outcome {
    fn error_code(&self) -> Option<ErrorCode> {
        match self {
            Self::Exited(0) => None,
            Self::Exited(_) | Self::Signaled(_) | Self::OutputOverflow | Self::IoFailure => {
                Some(ErrorCode::ArtifactInvalid)
            }
            Self::TimedOut | Self::Cancelled => Some(ErrorCode::BackendTimeout),
            Self::LaunchUnavailable => Some(ErrorCode::BackendUnavailable),
        }
    }
}

#[derive(Debug)]
struct Log {
    bytes: Vec<u8>,
    omitted: u64,
    truncated: bool,
}

impl Log {
    fn empty() -> Self {
        Self {
            bytes: Vec::new(),
            omitted: 0,
            truncated: false,
        }
    }

    fn append(&mut self, block: &[u8], cap: usize) {
        let keep = cap.saturating_sub(self.bytes.len()).min(block.len());
        self.bytes.extend_from_slice(&block[..keep]);
        self.omitted = self.omitted.saturating_add((block.len() - keep) as u64);
        self.truncated |= keep != block.len();
    }
}

#[derive(Debug)]
struct Termination {
    pidfd_opened: bool,
    leader_pinned_at_group_signal: bool,
    term_group_sent: bool,
    kill_group_sent: bool,
    direct_reaped: bool,
    stdout_eof: bool,
    stderr_eof: bool,
}

#[derive(Debug)]
struct Capture {
    outcome: Outcome,
    stdout: Log,
    stderr: Log,
    termination: Termination,
}

#[derive(Clone, Copy)]
struct Limits {
    wall: Duration,
    term_grace: Duration,
    drain_grace: Duration,
    max_output: usize,
}

fn set_nonblocking<T: std::os::fd::AsFd>(pipe: &T) -> io::Result<()> {
    let flags = fcntl_getfl(pipe).map_err(io::Error::from)?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK).map_err(io::Error::from)
}

fn drain<R: Read>(pipe: &mut Option<R>, log: &mut Log, cap: usize) -> io::Result<bool> {
    let mut block = [0_u8; 4096];
    for _ in 0..8 {
        let Some(reader) = pipe.as_mut() else { break };
        match reader.read(&mut block) {
            Ok(0) => {
                *pipe = None;
                return Ok(true);
            }
            Ok(count) => log.append(&block[..count], cap),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                *pipe = None;
                return Err(error);
            }
        }
    }
    Ok(false)
}

fn signal_group_if_pinned(pid: Pid, signal: Signal, pinned: bool) -> bool {
    pinned && kill_process_group(pid, signal).is_ok()
}

fn send_group(
    pid: Pid,
    pidfd: Option<&OwnedFd>,
    signal: Signal,
    child: &mut Child,
    exited: bool,
    group_pinned: bool,
) -> bool {
    let group = signal_group_if_pinned(pid, signal, group_pinned);
    if !exited {
        if let Some(fd) = pidfd {
            let _ = pidfd_send_signal(fd, signal);
        } else if signal == Signal::KILL && group_pinned {
            let _ = child.kill();
        }
    }
    group
}

fn launch_failure() -> Capture {
    Capture {
        outcome: Outcome::LaunchUnavailable,
        stdout: Log::empty(),
        stderr: Log::empty(),
        termination: Termination {
            pidfd_opened: false,
            leader_pinned_at_group_signal: false,
            term_group_sent: false,
            kill_group_sent: false,
            direct_reaped: false,
            stdout_eof: true,
            stderr_eof: true,
        },
    }
}

fn new_reason(
    previous: Option<Outcome>,
    completed: bool,
    cancelled: bool,
    overflow: bool,
    expired: bool,
) -> Option<Outcome> {
    if previous.is_some() {
        return previous;
    }
    if overflow {
        return Some(Outcome::OutputOverflow);
    }
    if completed {
        return None;
    }
    if cancelled {
        return Some(Outcome::Cancelled);
    }
    if expired {
        return Some(Outcome::TimedOut);
    }
    None
}

fn capture(command: Command, limits: Limits, cancel: &AtomicBool) -> Capture {
    capture_with_term_pause(command, limits, cancel, Duration::ZERO, |_| {})
}

fn capture_with_term_pause<F: FnOnce(Pid)>(
    mut command: Command,
    limits: Limits,
    cancel: &AtomicBool,
    term_pause: Duration,
    before_deadline: F,
) -> Capture {
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return launch_failure(),
    };
    let pid = Pid::from_raw(child.id() as i32).expect("spawned child has positive PID");
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).ok();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out_log = Log::empty();
    let mut err_log = Log::empty();
    let mut reason = None;
    let pipes_ready = !stdout
        .as_ref()
        .is_none_or(|pipe| set_nonblocking(pipe).is_err())
        && !stderr
            .as_ref()
            .is_none_or(|pipe| set_nonblocking(pipe).is_err());
    if !pipes_ready {
        stdout = None;
        stderr = None;
        reason = Some(Outcome::IoFailure);
    }
    before_deadline(pid);
    let started = Instant::now();
    let deadline = started + limits.wall;
    let mut stop_started = None;
    let mut term_group_sent = false;
    let mut kill_group_sent = false;
    let mut exited = false;
    let mut identity_pinned = true;
    let mut leader_pinned_at_group_signal = false;
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    loop {
        if !exited && identity_pinned {
            match waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            ) {
                Ok(status) => exited = status.is_some(),
                Err(_) => {
                    reason.get_or_insert(Outcome::IoFailure);
                    identity_pinned = false;
                }
            };
        }
        let stdout_read = drain(&mut stdout, &mut out_log, limits.max_output);
        let stderr_read = drain(&mut stderr, &mut err_log, limits.max_output);
        stdout_eof |= matches!(stdout_read, Ok(true));
        stderr_eof |= matches!(stderr_read, Ok(true));
        if stdout_read.is_err() || stderr_read.is_err() {
            reason.get_or_insert(Outcome::IoFailure);
        }
        let now = Instant::now();
        let completed = exited && stdout.is_none() && stderr.is_none();
        reason = new_reason(
            reason,
            completed,
            cancel.load(Ordering::Acquire),
            out_log.truncated || err_log.truncated,
            now >= deadline,
        );
        if reason.is_none() && completed {
            break;
        }
        if reason.is_some() && stop_started.is_none() {
            leader_pinned_at_group_signal |= identity_pinned && exited;
            term_group_sent = send_group(
                pid,
                pidfd.as_ref(),
                Signal::TERM,
                &mut child,
                exited,
                identity_pinned,
            );
            if !term_pause.is_zero() {
                thread::sleep(term_pause);
            }
            stop_started = Some(now);
        }
        if let Some(stopped) = stop_started {
            if now.duration_since(stopped) >= limits.term_grace && !kill_group_sent {
                leader_pinned_at_group_signal |= identity_pinned && exited;
                kill_group_sent = send_group(
                    pid,
                    pidfd.as_ref(),
                    Signal::KILL,
                    &mut child,
                    exited,
                    identity_pinned,
                );
            }
            if completed {
                break;
            }
            if now.duration_since(stopped) >= limits.term_grace + limits.drain_grace {
                leader_pinned_at_group_signal |= identity_pinned && exited;
                let _ = send_group(
                    pid,
                    pidfd.as_ref(),
                    Signal::KILL,
                    &mut child,
                    exited,
                    identity_pinned,
                );
                break;
            }
        }
        thread::sleep(Duration::from_millis(2));
    }
    out_log.truncated |= !stdout_eof;
    err_log.truncated |= !stderr_eof;
    let status: Option<ExitStatus> = child.try_wait().ok().flatten();
    let outcome = reason.unwrap_or_else(|| match status {
        Some(value) if value.code().is_some() => Outcome::Exited(value.code().unwrap()),
        Some(value) => Outcome::Signaled(value.signal().unwrap_or_default()),
        None => Outcome::IoFailure,
    });
    Capture {
        outcome,
        stdout: out_log,
        stderr: err_log,
        termination: Termination {
            pidfd_opened: pidfd.is_some(),
            leader_pinned_at_group_signal,
            term_group_sent,
            kill_group_sent,
            direct_reaped: status.is_some(),
            stdout_eof,
            stderr_eof,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Instant;

    #[test]
    fn fixture_process() {
        let Ok(mode) = std::env::var("ARCHY_PROCESS_FIXTURE_MODE") else {
            return;
        };
        match mode.as_str() {
            "fast" => {
                std::io::stdout().write_all(b"out").unwrap();
                std::io::stderr().write_all(b"err").unwrap();
            }
            "hang" => thread::sleep(Duration::from_secs(30)),
            "exit7" => std::process::exit(7),
            "flood" => loop {
                std::io::stdout().write_all(&[b'x'; 8192]).unwrap();
                std::io::stderr().write_all(&[b'y'; 8192]).unwrap();
            },
            _ => panic!("unknown fixture mode"),
        }
    }

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "process::tests::fixture_process", "--nocapture"])
            .env("ARCHY_PROCESS_FIXTURE_MODE", mode);
        command
    }

    fn limits() -> Limits {
        Limits {
            wall: Duration::from_millis(250),
            term_grace: Duration::from_millis(80),
            drain_grace: Duration::from_millis(150),
            max_output: 1024,
        }
    }

    struct EscapedFixture {
        dir: PathBuf,
        pidfd: Option<OwnedFd>,
    }

    impl EscapedFixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "archy-escaped-{}-{:?}",
                std::process::id(),
                thread::current().id()
            ));
            std::fs::create_dir(&dir).unwrap();
            Self { dir, pidfd: None }
        }

        fn pin(&mut self) -> Pid {
            let until = Instant::now() + Duration::from_secs(3);
            while !std::fs::metadata(self.dir.join("pid")).is_ok_and(|meta| meta.len() > 0) {
                assert!(Instant::now() < until, "escaped child did not become ready");
                thread::sleep(Duration::from_millis(2));
            }
            let text = std::fs::read_to_string(self.dir.join("pid")).unwrap();
            let pid = Pid::from_raw(text.trim().parse().unwrap()).unwrap();
            self.pidfd = Some(pidfd_open(pid, PidfdFlags::empty()).unwrap());
            std::fs::write(self.dir.join("go"), b"").unwrap();
            while !self.dir.join("ready").exists() {
                assert!(
                    Instant::now() < until,
                    "escaped child did not enter its session"
                );
                thread::sleep(Duration::from_millis(2));
            }
            pid
        }

        fn release(&self) {
            std::fs::write(self.dir.join("ack"), b"").unwrap();
        }
    }

    impl Drop for EscapedFixture {
        fn drop(&mut self) {
            if let Some(fd) = &self.pidfd {
                let _ = pidfd_send_signal(fd, Signal::KILL);
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn read_error_closes_pipe_without_claiming_eof() {
        struct BadReader;
        impl Read for BadReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("broken pipe read"))
            }
        }
        let mut pipe = Some(BadReader);
        let result = drain(&mut pipe, &mut Log::empty(), 1024);
        assert!(result.is_err());
        assert!(pipe.is_none());
    }

    #[test]
    fn fast_exit_reaps_and_captures_both_streams() {
        let result = capture(fixture("fast"), limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::Exited(0));
        assert_eq!(result.outcome.error_code(), None);
        assert!(result.stdout.bytes.windows(3).any(|part| part == b"out"));
        assert!(result.stderr.bytes.windows(3).any(|part| part == b"err"));
        assert!(result.termination.direct_reaped);
        assert!(result.termination.pidfd_opened || result.termination.direct_reaped);
        assert!(result.termination.stdout_eof && result.termination.stderr_eof);
    }

    #[test]
    fn exit_signal_and_launch_failure_are_distinct() {
        let failed = capture(fixture("exit7"), limits(), &AtomicBool::new(false));
        assert_eq!(failed.outcome, Outcome::Exited(7));
        assert_eq!(
            failed.outcome.error_code(),
            Some(ErrorCode::ArtifactInvalid)
        );
        let mut signaled = Command::new("/bin/sh");
        signaled.args(["-c", "kill -TERM $$"]);
        let result = capture(signaled, limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::Signaled(Signal::TERM.as_raw()));
        assert!(result.termination.direct_reaped);
        let unavailable = capture(
            Command::new("/nonexistent/archy-process-fixture"),
            limits(),
            &AtomicBool::new(false),
        );
        assert_eq!(unavailable.outcome, Outcome::LaunchUnavailable);
        assert_eq!(
            unavailable.outcome.error_code(),
            Some(ErrorCode::BackendUnavailable)
        );
    }

    #[test]
    fn hung_child_times_out_and_is_reaped() {
        let start = Instant::now();
        let result = capture(fixture("hang"), limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::TimedOut);
        assert_eq!(result.outcome.error_code(), Some(ErrorCode::BackendTimeout));
        assert!(result.termination.direct_reaped);
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn term_ignoring_child_is_killed_and_reaped() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "trap '' TERM; echo ready; exec /bin/sleep 30"]);
        let result = capture(command, limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::TimedOut);
        assert!(result.termination.term_group_sent && result.termination.kill_group_sent);
        assert!(result.termination.direct_reaped);
    }

    #[test]
    fn flood_truncates_without_deadlock() {
        let result = capture(fixture("flood"), limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::OutputOverflow);
        assert_eq!(
            result.outcome.error_code(),
            Some(ErrorCode::ArtifactInvalid)
        );
        assert!(result.stdout.bytes.len() <= 1024 && result.stderr.bytes.len() <= 1024);
        assert!(result.stdout.truncated || result.stderr.truncated);
        assert!(result.stdout.omitted > 0 || result.stderr.omitted > 0);
        assert!(result.termination.direct_reaped);
    }

    #[test]
    fn descendant_held_pipe_has_bounded_teardown() {
        let start = Instant::now();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "/bin/sleep 30 & echo descendant:$!; exit 0"]);
        let result = capture(command, limits(), &AtomicBool::new(false));
        assert_eq!(result.outcome, Outcome::TimedOut);
        assert!(result.termination.direct_reaped);
        assert!(result.termination.kill_group_sent || result.termination.term_group_sent);
        assert!(result.termination.stdout_eof && result.termination.stderr_eof);
        let text = String::from_utf8_lossy(&result.stdout.bytes);
        let pid = text
            .split("descendant:")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse::<u32>()
            .unwrap();
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            let state = stat.split(") ").nth(1).unwrap().chars().next().unwrap();
            assert_eq!(state, 'Z', "descendant still running: {pid}");
        }
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn escaped_session_pipeholder_does_not_lose_group_identity_before_signaling() {
        let mut escaped = EscapedFixture::new();
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            r#"/bin/sh -c 'while [ ! -e "$1/go" ]; do /bin/sleep 0.01; done; exec /usr/bin/setsid /bin/sh -c '\''echo ready > "$1/ready"; while [ ! -e "$1/ack" ]; do /bin/sleep 0.01; done; exec /bin/sleep 10'\'' sh "$1"' sh "$ARCHY_ESCAPED_DIR" & escaped=$!; echo "$escaped" > "$ARCHY_ESCAPED_DIR/pid"; while [ ! -e "$ARCHY_ESCAPED_DIR/ack" ]; do /bin/sleep 0.01; done; echo escaped:$escaped; exit 0"#,
        ]);
        command.env("ARCHY_ESCAPED_DIR", &escaped.dir);
        let (result, pid) = thread::scope(|scope| {
            let (leader_tx, leader_rx) = mpsc::channel();
            let (start_tx, start_rx) = mpsc::channel();
            let capture_thread = scope.spawn(move || {
                capture_with_term_pause(
                    command,
                    limits(),
                    &AtomicBool::new(false),
                    Duration::from_millis(1200),
                    |leader| {
                        let _ = leader_tx.send(leader);
                        let _ = start_rx.recv();
                    },
                )
            });
            let leader = leader_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            let _leader_pidfd = pidfd_open(leader, PidfdFlags::empty()).unwrap();
            let pid = escaped.pin();
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            let fields: Vec<_> = stat
                .split(") ")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .collect();
            assert_ne!(fields[0], "Z", "escaped child exited before release");
            assert_eq!(fields[3].parse::<i32>().unwrap(), pid.as_raw_pid());
            escaped.release();
            let until = Instant::now() + Duration::from_secs(3);
            loop {
                let stat = std::fs::read_to_string(format!("/proc/{leader}/stat")).unwrap();
                if stat.split(") ").nth(1).unwrap().starts_with('Z') {
                    break;
                }
                assert!(Instant::now() < until, "group leader did not exit");
                thread::sleep(Duration::from_millis(2));
            }
            start_tx.send(()).unwrap();
            (capture_thread.join().unwrap(), pid)
        });
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let fields: Vec<_> = stat
            .split(") ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .collect();
        assert_ne!(
            fields[0], "Z",
            "escaped child exited before capture returned"
        );
        assert_eq!(result.outcome, Outcome::TimedOut);
        assert!(result.termination.pidfd_opened);
        assert!(result.termination.leader_pinned_at_group_signal);
        assert!(result.termination.term_group_sent);
        assert!(result.termination.direct_reaped);
        assert!(!result.termination.stdout_eof || !result.termination.stderr_eof);
        let text = String::from_utf8_lossy(&result.stdout.bytes);
        let captured_pid = text
            .split("escaped:")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        assert_eq!(captured_pid, pid.as_raw_pid());
    }

    #[test]
    fn no_group_signal_when_leader_identity_is_unavailable() {
        let mut command = Command::new("/bin/sleep");
        command.arg("1").process_group(0);
        let mut child = command.spawn().unwrap();
        let pid = Pid::from_raw(child.id() as i32).unwrap();
        assert!(!signal_group_if_pinned(pid, Signal::TERM, false));
        assert!(child.try_wait().unwrap().is_none());
        child.kill().unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < until, "fixture child was not reaped");
            thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn completed_exit_wins_new_cancel_or_deadline_but_preserves_prior_failure() {
        assert_eq!(new_reason(None, true, true, false, true), None);
        assert_eq!(new_reason(None, true, false, false, true), None);
        assert_eq!(
            new_reason(None, true, true, true, true),
            Some(Outcome::OutputOverflow)
        );
        assert_eq!(
            new_reason(Some(Outcome::OutputOverflow), true, true, false, true),
            Some(Outcome::OutputOverflow)
        );
        assert_eq!(
            new_reason(Some(Outcome::IoFailure), true, true, false, true),
            Some(Outcome::IoFailure)
        );
    }

    #[test]
    fn cancellation_terminates_child() {
        let flag = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&flag);
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(60));
            setter.store(true, Ordering::Release);
        });
        let result = capture(fixture("hang"), limits(), &flag);
        thread.join().unwrap();
        assert_eq!(result.outcome, Outcome::Cancelled);
        assert_eq!(result.outcome.error_code(), Some(ErrorCode::BackendTimeout));
        assert!(result.termination.direct_reaped);
    }
}

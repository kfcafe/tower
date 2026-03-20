use std::io::{BufRead, BufReader, Read};
use std::process::Child;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Configuration for process timeout monitoring.
#[derive(Debug, Clone, Default)]
pub struct TimeoutConfig {
    /// Maximum total wall-clock time the process may run.
    pub total_timeout: Duration,
    /// Maximum time allowed between consecutive lines of stdout output.
    pub idle_timeout: Duration,
}

/// Result of monitoring a child process.
#[derive(Debug, PartialEq)]
pub enum MonitorResult {
    /// Process exited on its own.
    Completed,
    /// Total timeout exceeded — process was killed.
    TotalTimeout,
    /// Idle timeout exceeded (no output) — process was killed.
    IdleTimeout,
}

/// Monitor a child process's stdout, enforcing total and idle timeouts.
///
/// Reads stdout line-by-line via a background reader thread. On each line the
/// idle timer is reset and `on_line` is called. If the total elapsed time or
/// idle time exceeds the configured limits the process is killed with SIGKILL
/// and the corresponding [`MonitorResult`] is returned.
///
/// `stdout` is passed separately so the caller can `child.stdout.take()` and
/// hand it in while retaining ownership of the `Child` (needed to call
/// `kill`/`wait`).
pub fn monitor_process<R: Read + Send + 'static>(
    child: &mut Child,
    stdout: R,
    config: &TimeoutConfig,
    mut on_line: impl FnMut(&str),
) -> MonitorResult {
    let start = Instant::now();
    let mut last_activity = Instant::now();

    // Channel-based approach: a background thread reads lines and sends them
    // over a channel so the main thread can poll with a timeout.
    let (tx, rx) = mpsc::channel::<Option<String>>();

    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    if tx.send(Some(text)).is_err() {
                        break; // receiver dropped
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(None); // signal EOF
    });

    // Poll interval: use the smaller of the two timeouts (if set), capped at
    // 50ms so we stay responsive without busy-spinning.
    let poll = poll_interval(config);

    loop {
        // Check total timeout.
        if !config.total_timeout.is_zero() && start.elapsed() > config.total_timeout {
            kill_process(child);
            return MonitorResult::TotalTimeout;
        }

        // Check idle timeout.
        if !config.idle_timeout.is_zero() && last_activity.elapsed() > config.idle_timeout {
            kill_process(child);
            return MonitorResult::IdleTimeout;
        }

        match rx.recv_timeout(poll) {
            Ok(Some(text)) => {
                last_activity = Instant::now();
                on_line(&text);
            }
            Ok(None) => {
                // EOF — process closed stdout.
                let _ = child.wait();
                return MonitorResult::Completed;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No line yet — loop back and check timeouts.
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread exited without sending EOF.
                let _ = child.wait();
                return MonitorResult::Completed;
            }
        }
    }
}

/// Compute a reasonable poll interval from the timeout config.
fn poll_interval(config: &TimeoutConfig) -> Duration {
    let mut interval = Duration::from_millis(50);
    if !config.idle_timeout.is_zero() {
        interval = interval.min(config.idle_timeout / 4);
    }
    if !config.total_timeout.is_zero() {
        interval = interval.min(config.total_timeout / 4);
    }
    // Floor at 5ms to avoid busy-spinning.
    interval.max(Duration::from_millis(5))
}

/// Kill a child process with SIGKILL and reap it.
fn kill_process(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn timeout_completed_fast_process() {
        let mut child = Command::new("echo")
            .arg("hello")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let config = TimeoutConfig {
            total_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(5),
        };

        let mut lines = Vec::new();
        let result = monitor_process(&mut child, stdout, &config, |line| {
            lines.push(line.to_string());
        });

        assert_eq!(result, MonitorResult::Completed);
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn timeout_total_timeout_kills_process() {
        let mut child = Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let config = TimeoutConfig {
            total_timeout: Duration::from_millis(100),
            idle_timeout: Duration::ZERO,
        };

        let result = monitor_process(&mut child, stdout, &config, |_| {});
        assert_eq!(result, MonitorResult::TotalTimeout);
    }

    #[test]
    fn timeout_idle_timeout_kills_slow_writer() {
        let mut child = Command::new("bash")
            .args(["-c", "echo start; sleep 60"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let config = TimeoutConfig {
            total_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_millis(200),
        };

        let mut lines = Vec::new();
        let result = monitor_process(&mut child, stdout, &config, |line| {
            lines.push(line.to_string());
        });

        assert_eq!(lines, vec!["start"]);
        assert_eq!(result, MonitorResult::IdleTimeout);
    }

    #[test]
    fn timeout_zero_timeouts_means_no_limit() {
        let mut child = Command::new("bash")
            .args(["-c", "echo a; echo b; echo c"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let config = TimeoutConfig::default();

        let mut lines = Vec::new();
        let result = monitor_process(&mut child, stdout, &config, |line| {
            lines.push(line.to_string());
        });

        assert_eq!(result, MonitorResult::Completed);
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn timeout_callback_receives_all_lines() {
        let mut child = Command::new("bash")
            .args(["-c", "for i in 1 2 3 4 5; do echo line$i; done"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let config = TimeoutConfig {
            total_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(5),
        };

        let mut lines = Vec::new();
        let result = monitor_process(&mut child, stdout, &config, |line| {
            lines.push(line.to_string());
        });

        assert_eq!(result, MonitorResult::Completed);
        assert_eq!(lines, vec!["line1", "line2", "line3", "line4", "line5"]);
    }
}

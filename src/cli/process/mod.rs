//! Supervised execution of one external process: spawn in its own process
//! group, drain stdout and stderr concurrently under an output limit, feed
//! stdin, enforce an optional deadline, forward SIGINT and SIGTERM, and
//! always reap the child. Cleanup is best-effort within the direct child's
//! process group; descendants that open their own group or session are
//! outside the guarantee, and non-Unix builds signal only the direct child.
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_TERMINATION_GRACE: Duration = Duration::from_secs(2);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessTermination {
    Exited(i32),
    Signaled(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessOutput {
    pub(crate) termination: ProcessTermination,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl ProcessOutput {
    pub(crate) fn code(&self) -> Option<i32> {
        match self.termination {
            ProcessTermination::Exited(code) => Some(code),
            ProcessTermination::Signaled(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessFailure {
    Spawn(String),
    Wait(String),
    Stdin(String),
    Stdout(String),
    Stderr(String),
    Timeout(Duration),
    ForwardedSignal(i32),
    OutputLimit {
        stream: &'static str,
        limit: usize,
        tail: String,
    },
}

impl fmt::Display for ProcessFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(message)
            | Self::Wait(message)
            | Self::Stdin(message)
            | Self::Stdout(message)
            | Self::Stderr(message) => formatter.write_str(message),
            Self::Timeout(timeout) => write!(
                formatter,
                "process timed out after {} seconds",
                timeout.as_secs_f64()
            ),
            Self::ForwardedSignal(signal) => {
                write!(
                    formatter,
                    "process stopped after forwarding signal {signal}"
                )
            }
            Self::OutputLimit {
                stream,
                limit,
                tail,
            } => {
                write!(
                    formatter,
                    "process {stream} exceeded the {limit}-byte output limit"
                )?;
                if !tail.trim().is_empty() {
                    let diagnostic: String = tail.trim().chars().take(500).collect();
                    write!(formatter, ": {diagnostic}")?;
                }
                Ok(())
            }
        }
    }
}

pub(crate) struct ProcessRequest {
    pub(crate) binary: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) env_remove: Vec<OsString>,
    pub(crate) stdin: Option<Vec<u8>>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) output_limit: usize,
    pub(crate) termination_grace: Duration,
}

impl ProcessRequest {
    pub(crate) fn new(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
            args: Vec::new(),
            current_dir: None,
            env: Vec::new(),
            env_remove: Vec::new(),
            stdin: None,
            timeout: None,
            output_limit: DEFAULT_OUTPUT_LIMIT_BYTES,
            termination_grace: DEFAULT_TERMINATION_GRACE,
        }
    }
}

struct CapturedStream {
    tail: VecDeque<u8>,
    exceeded_limit: bool,
}

impl CapturedStream {
    fn into_string(self) -> String {
        String::from_utf8_lossy(&self.tail.into_iter().collect::<Vec<_>>()).into_owned()
    }
}

struct SignalFlags {
    interrupt: Arc<AtomicBool>,
    terminate: Arc<AtomicBool>,
    registrations: Vec<signal_hook::SigId>,
}

impl SignalFlags {
    #[cfg(unix)]
    fn register() -> Result<Self, ProcessFailure> {
        use signal_hook::consts::signal::{SIGINT, SIGTERM};

        let interrupt = Arc::new(AtomicBool::new(false));
        let terminate = Arc::new(AtomicBool::new(false));
        let interrupt_registration = signal_hook::flag::register(SIGINT, Arc::clone(&interrupt))
            .map_err(|error| {
                ProcessFailure::Wait(format!("cannot register SIGINT handler: {error}"))
            })?;
        let terminate_registration =
            match signal_hook::flag::register(SIGTERM, Arc::clone(&terminate)) {
                Ok(registration) => registration,
                Err(error) => {
                    signal_hook::low_level::unregister(interrupt_registration);
                    return Err(ProcessFailure::Wait(format!(
                        "cannot register SIGTERM handler: {error}"
                    )));
                }
            };
        Ok(Self {
            interrupt,
            terminate,
            registrations: vec![interrupt_registration, terminate_registration],
        })
    }

    #[cfg(not(unix))]
    fn register() -> Result<Self, ProcessFailure> {
        Ok(Self {
            interrupt: Arc::new(AtomicBool::new(false)),
            terminate: Arc::new(AtomicBool::new(false)),
            registrations: Vec::new(),
        })
    }

    fn received(&self) -> Option<i32> {
        #[cfg(unix)]
        {
            use signal_hook::consts::signal::{SIGINT, SIGTERM};

            if self.interrupt.load(Ordering::Relaxed) {
                return Some(SIGINT);
            }
            if self.terminate.load(Ordering::Relaxed) {
                return Some(SIGTERM);
            }
        }
        None
    }
}

impl Drop for SignalFlags {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            signal_hook::low_level::unregister(registration);
        }
    }
}

fn spawn_command(request: &ProcessRequest) -> Command {
    let mut command = Command::new(&request.binary);
    command.args(&request.args);
    if let Some(current_dir) = &request.current_dir {
        command.current_dir(current_dir);
    }
    command.envs(request.env.iter().cloned());
    for name in &request.env_remove {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

fn reader_thread<R: Read + Send + 'static>(
    stream_name: &'static str,
    source: Option<R>,
    limit: usize,
    exceeded_limit: Arc<AtomicBool>,
) -> std::thread::JoinHandle<Result<CapturedStream, ProcessFailure>> {
    std::thread::spawn(move || {
        let mut source = source.ok_or_else(|| {
            let message = format!("cannot capture process {stream_name}: pipe is unavailable");
            if stream_name == "stdout" {
                ProcessFailure::Stdout(message)
            } else {
                ProcessFailure::Stderr(message)
            }
        })?;
        let mut tail = VecDeque::with_capacity(limit.min(64 * 1024));
        let mut total = 0usize;
        let mut chunk = [0u8; 8192];
        loop {
            let bytes_read = source.read(&mut chunk).map_err(|error| {
                let message = format!("cannot read process {stream_name}: {error}");
                if stream_name == "stdout" {
                    ProcessFailure::Stdout(message)
                } else {
                    ProcessFailure::Stderr(message)
                }
            })?;
            if bytes_read == 0 {
                break;
            }
            total = total.saturating_add(bytes_read);
            if total > limit {
                exceeded_limit.store(true, Ordering::Relaxed);
            }
            for byte in &chunk[..bytes_read] {
                if tail.len() == limit {
                    tail.pop_front();
                }
                if limit > 0 {
                    tail.push_back(*byte);
                }
            }
        }
        Ok(CapturedStream {
            tail,
            exceeded_limit: total > limit,
        })
    })
}

fn writer_thread(
    stdin: Option<std::process::ChildStdin>,
    content: Vec<u8>,
) -> std::thread::JoinHandle<Result<(), ProcessFailure>> {
    std::thread::spawn(move || {
        use std::io::Write;

        let mut stdin = stdin.ok_or_else(|| {
            ProcessFailure::Stdin("cannot write process stdin: pipe is unavailable".to_string())
        })?;
        match stdin.write_all(&content) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(ProcessFailure::Stdin(format!(
                "cannot write process stdin: {error}"
            ))),
        }
    })
}

#[cfg(unix)]
fn send_process_group_signal(child: &Child, signal: i32) -> Result<(), ProcessFailure> {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let signal = Signal::try_from(signal)
        .map_err(|error| ProcessFailure::Wait(format!("invalid process signal: {error}")))?;
    match killpg(Pid::from_raw(child.id().cast_signed()), signal) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(ProcessFailure::Wait(format!(
            "cannot signal process group {}: {error}",
            child.id()
        ))),
    }
}

#[cfg(not(unix))]
fn send_process_group_signal(child: &mut Child, _signal: i32) -> Result<(), ProcessFailure> {
    child
        .kill()
        .map_err(|error| ProcessFailure::Wait(format!("cannot stop process: {error}")))
}

fn reap_after_failure(
    child: &mut Child,
    failure: ProcessFailure,
) -> Result<ExitStatus, ProcessFailure> {
    let kill_error = child.kill().err();
    match child.wait() {
        Ok(_) => Err(failure),
        Err(wait_error) => {
            let kill_detail = kill_error.map_or_else(String::new, |error| {
                format!("; direct child kill also failed: {error}")
            });
            Err(ProcessFailure::Wait(format!(
                "{failure}{kill_detail}; cannot reap process: {wait_error}"
            )))
        }
    }
}

fn wait_after_signal(
    child: &mut Child,
    initial_signal: i32,
    grace: Duration,
) -> Result<ExitStatus, ProcessFailure> {
    #[cfg(unix)]
    if let Err(failure) = send_process_group_signal(child, initial_signal) {
        return reap_after_failure(child, failure);
    }
    #[cfg(not(unix))]
    if let Err(failure) = send_process_group_signal(child, initial_signal) {
        return reap_after_failure(child, failure);
    }

    let deadline = Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(PROCESS_POLL_INTERVAL);
            }
            Ok(None) => break,
            Err(error) => {
                return reap_after_failure(
                    child,
                    ProcessFailure::Wait(format!("cannot wait for process after signal: {error}")),
                );
            }
        }
    }

    #[cfg(unix)]
    {
        use signal_hook::consts::signal::SIGKILL;
        if let Err(failure) = send_process_group_signal(child, SIGKILL) {
            return reap_after_failure(child, failure);
        }
    }
    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|error| ProcessFailure::Wait(format!("cannot kill process: {error}")))?;
    child
        .wait()
        .map_err(|error| ProcessFailure::Wait(format!("cannot reap process: {error}")))
}

fn termination_from_status(status: ExitStatus) -> ProcessTermination {
    if let Some(code) = status.code() {
        return ProcessTermination::Exited(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ProcessTermination::Signaled(status.signal().unwrap_or_default())
    }
    #[cfg(not(unix))]
    ProcessTermination::Signaled(0)
}

fn join_writer(
    handle: Option<std::thread::JoinHandle<Result<(), ProcessFailure>>>,
) -> Result<(), ProcessFailure> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .map_err(|_| ProcessFailure::Stdin("process stdin writer panicked".to_string()))?
}

fn join_reader(
    stream_name: &'static str,
    handle: std::thread::JoinHandle<Result<CapturedStream, ProcessFailure>>,
) -> Result<CapturedStream, ProcessFailure> {
    handle.join().map_err(|_| {
        let message = format!("process {stream_name} reader panicked");
        if stream_name == "stdout" {
            ProcessFailure::Stdout(message)
        } else {
            ProcessFailure::Stderr(message)
        }
    })?
}

fn requested_stop(
    request: &ProcessRequest,
    signal_flags: &SignalFlags,
    stdout_exceeded: &AtomicBool,
    stderr_exceeded: &AtomicBool,
    started: Instant,
) -> Option<ProcessFailure> {
    if stdout_exceeded.load(Ordering::Relaxed) {
        return Some(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: request.output_limit,
            tail: String::new(),
        });
    }
    if stderr_exceeded.load(Ordering::Relaxed) {
        return Some(ProcessFailure::OutputLimit {
            stream: "stderr",
            limit: request.output_limit,
            tail: String::new(),
        });
    }
    if let Some(signal) = signal_flags.received() {
        return Some(ProcessFailure::ForwardedSignal(signal));
    }
    request
        .timeout
        .filter(|timeout| started.elapsed() >= *timeout)
        .map(ProcessFailure::Timeout)
}

fn stopping_signal(failure: &ProcessFailure) -> i32 {
    #[cfg(unix)]
    {
        use signal_hook::consts::signal::SIGTERM;

        match failure {
            ProcessFailure::ForwardedSignal(signal) => *signal,
            _ => SIGTERM,
        }
    }
    #[cfg(not(unix))]
    0
}

fn wait_for_process(
    child: &mut Child,
    request: &ProcessRequest,
    signal_flags: &SignalFlags,
    stdout_exceeded: &AtomicBool,
    stderr_exceeded: &AtomicBool,
    binary: &str,
) -> Result<(ExitStatus, Option<ProcessFailure>), ProcessFailure> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, None)),
            Ok(None) => {}
            Err(error) => {
                let failure = ProcessFailure::Wait(format!("{binary} wait failed: {error}"));
                let status =
                    wait_after_signal(child, stopping_signal(&failure), request.termination_grace)?;
                return Ok((status, Some(failure)));
            }
        }

        if let Some(failure) = requested_stop(
            request,
            signal_flags,
            stdout_exceeded,
            stderr_exceeded,
            started,
        ) {
            let status =
                wait_after_signal(child, stopping_signal(&failure), request.termination_grace)?;
            return Ok((status, Some(failure)));
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

pub(crate) fn run_process_request(
    request: &ProcessRequest,
) -> Result<ProcessOutput, ProcessFailure> {
    let binary = request.binary.to_string_lossy();
    let mut command = spawn_command(request);
    command
        .stdin(if request.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let signal_flags = SignalFlags::register()?;
    let mut child = command
        .spawn()
        .map_err(|error| ProcessFailure::Spawn(format!("{binary} not runnable: {error}")))?;
    let stdout_exceeded = Arc::new(AtomicBool::new(false));
    let stderr_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_handle = reader_thread(
        "stdout",
        child.stdout.take(),
        request.output_limit,
        Arc::clone(&stdout_exceeded),
    );
    let stderr_handle = reader_thread(
        "stderr",
        child.stderr.take(),
        request.output_limit,
        Arc::clone(&stderr_exceeded),
    );
    let stdin_handle = request
        .stdin
        .clone()
        .map(|content| writer_thread(child.stdin.take(), content));
    let wait_result = wait_for_process(
        &mut child,
        request,
        &signal_flags,
        &stdout_exceeded,
        &stderr_exceeded,
        &binary,
    );
    let stdin_result = join_writer(stdin_handle);
    let stdout_result = join_reader("stdout", stdout_handle);
    let stderr_result = join_reader("stderr", stderr_handle);

    let (status, stop_failure) = wait_result?;
    stdin_result?;
    let stdout_capture = stdout_result?;
    let stderr_capture = stderr_result?;
    let stdout_exceeded_limit = stdout_capture.exceeded_limit;
    let stderr_exceeded_limit = stderr_capture.exceeded_limit;
    let stdout = stdout_capture.into_string();
    let stderr = stderr_capture.into_string();
    if let Some(mut failure) = stop_failure {
        if let ProcessFailure::OutputLimit { stream, tail, .. } = &mut failure {
            *tail = if *stream == "stdout" {
                stdout.clone()
            } else {
                stderr.clone()
            };
        }
        return Err(failure);
    }
    if stdout_exceeded_limit {
        return Err(ProcessFailure::OutputLimit {
            stream: "stdout",
            limit: request.output_limit,
            tail: stdout,
        });
    }
    if stderr_exceeded_limit {
        return Err(ProcessFailure::OutputLimit {
            stream: "stderr",
            limit: request.output_limit,
            tail: stderr,
        });
    }
    Ok(ProcessOutput {
        termination: termination_from_status(status),
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests;

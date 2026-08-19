use super::*;

fn shell_request(script: &str) -> ProcessRequest {
    let mut request = ProcessRequest::new("/bin/sh");
    request.args = vec![OsString::from("-c"), OsString::from(script)];
    request
}

#[test]
fn process_request_has_no_default_timeout() {
    let request = ProcessRequest::new("/bin/true");
    assert_eq!(request.timeout, None);
}

#[test]
fn process_captures_stdout_stderr_and_exit_code() {
    let request = shell_request("printf answer; printf warning >&2; exit 7");
    let output = run_process_request(&request).expect("process output");

    assert_eq!(output.termination, ProcessTermination::Exited(7));
    assert_eq!(output.stdout, "answer");
    assert_eq!(output.stderr, "warning");
}

#[test]
fn process_captures_stdin_without_deadlock() {
    let mut request = ProcessRequest::new("/bin/cat");
    request.stdin = Some(b"question\n".to_vec());
    let output = run_process_request(&request).expect("process output");

    assert_eq!(output.termination, ProcessTermination::Exited(0));
    assert_eq!(output.stdout, "question\n");
}

#[test]
fn process_preserves_exit_when_child_closes_stdin_early() {
    let mut request = shell_request("exit 23");
    request.stdin = Some(vec![b'x'; DEFAULT_OUTPUT_LIMIT_BYTES * 2]);
    let output = run_process_request(&request).expect("process output");

    assert_eq!(output.termination, ProcessTermination::Exited(23));
}

#[test]
fn process_reports_signal_termination() {
    let request = shell_request("kill -TERM $$");
    let output = run_process_request(&request).expect("process output");

    assert!(matches!(
        output.termination,
        ProcessTermination::Signaled(signal)
            if signal == signal_hook::consts::signal::SIGTERM
    ));
}

#[test]
fn process_timeout_uses_grace_then_reports_timeout() {
    let mut request = shell_request("trap '' TERM; sleep 5");
    request.stdin = Some(vec![b'x'; DEFAULT_OUTPUT_LIMIT_BYTES * 2]);
    request.timeout = Some(Duration::from_millis(50));
    request.termination_grace = Duration::from_millis(50);
    let started = Instant::now();
    let failure = run_process_request(&request).expect_err("timeout");

    assert_eq!(failure, ProcessFailure::Timeout(Duration::from_millis(50)));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn process_drains_output_after_limit_is_crossed() {
    let mut request =
        shell_request("i=0; while [ \"$i\" -lt 100 ]; do printf 0123456789; i=$((i + 1)); done");
    request.output_limit = 64;
    request.termination_grace = Duration::from_millis(50);
    let failure = run_process_request(&request).expect_err("output limit");

    let ProcessFailure::OutputLimit {
        stream,
        limit,
        tail,
    } = failure
    else {
        panic!("expected output limit failure");
    };
    assert_eq!(stream, "stdout");
    assert_eq!(limit, 64);
    assert_eq!(tail, format!("6789{}", "0123456789".repeat(6)));
}

#[test]
fn process_request_removes_inherited_automation_mode() {
    let mut request = shell_request("printf %s \"${HARNESS_AUTOMATED-unset}\"");
    request.env = vec![(OsString::from("HARNESS_AUTOMATED"), OsString::from("1"))];
    request.env_remove = vec![OsString::from("HARNESS_AUTOMATED")];
    let output = run_process_request(&request).expect("process output");

    assert_eq!(output.termination, ProcessTermination::Exited(0));
    assert_eq!(output.stdout, "unset");
}

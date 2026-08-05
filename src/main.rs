mod cli;
#[cfg(feature = "tui")]
mod tui;

/// A consumer closing the pipe early (`rune … | head`, `| grep -q`) is normal
/// stream consumption, not a crash. `println!` panics on the resulting
/// `EPIPE`; without `unsafe` the disposition cannot be reset to `SIG_DFL`,
/// so the panic hook recognizes that one payload and exits quietly with the
/// conventional SIGPIPE status.
fn exit_quietly_on_broken_pipe(panic_info: &std::panic::PanicHookInfo<'_>) {
    let payload = panic_info.payload();
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or_default();
    if message.contains("Broken pipe") {
        std::process::exit(141);
    }
}

fn main() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        exit_quietly_on_broken_pipe(panic_info);
        default_hook(panic_info);
    }));
    let exit_code = cli::run();
    std::process::exit(exit_code);
}

#![forbid(unsafe_code)]

fn main() {
    let debug_logging = std::env::args_os().any(|arg| arg == "--verbose" || arg == "--debug");

    // Initialise logging. Debug logs are disabled unless --verbose or --debug is passed.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(if debug_logging {
                    "numbr_ui=debug"
                } else {
                    "numbr_ui=info"
                })
            }),
        )
        .init();

    if let Err(e) = numbr_ui::app::run() {
        eprintln!("numbr: {e}");
        std::process::exit(1);
    }
}

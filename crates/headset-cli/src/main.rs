use anyhow::Result;
use clap::Parser;
use headset_cli::{cli, cmd, redact::Redactor};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("HEADSETCTL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = cli::Cli::parse();
    let r = Redactor::new(args.include_sensitive);

    #[cfg(windows)]
    let backend = headset_device::WindowsHidBackend::new();
    #[cfg(not(windows))]
    compile_error!("headsetctl targets Windows only");

    let out = match &args.command {
        cli::Command::List(a) => cmd::list::run(&backend, a, &r, args.json)?,
        cli::Command::Inspect(a) => cmd::inspect::run(&backend, a, &r, args.json)?,
        cli::Command::Probe(_) => "probe: not implemented until Task 10".to_string(),
    };
    print!("{out}");
    Ok(())
}

mod cli;
mod redact;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("HEADSETCTL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = cli::Cli::parse();
    let _ = redact::Redactor::new(args.include_sensitive);
    match args.command {
        cli::Command::List(_) => println!("list: not implemented until Task 7"),
        cli::Command::Inspect(_) => println!("inspect: not implemented until Task 7"),
        cli::Command::Probe(_) => println!("probe: not implemented until Task 10"),
    }
    Ok(())
}

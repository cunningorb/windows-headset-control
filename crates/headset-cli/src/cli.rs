use clap::{Args, Parser, Subcommand};

/// Accepts `0x1532`, `1532`, and `101b`. Always hexadecimal: USB IDs are
/// universally written in hex, so a decimal reading would silently mislead.
pub fn parse_u16_id(s: &str) -> Result<u16, String> {
    let t = s.trim();
    let body = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("`{s}` is not a hexadecimal USB id"));
    }
    u16::from_str_radix(body, 16).map_err(|_| format!("`{s}` does not fit in 16 bits"))
}

#[derive(Parser, Debug)]
#[command(
    name = "headsetctl",
    about = "Experimental native Windows HID controller for supported wireless headset settings.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Reveal device paths. Output will contain machine-identifying data.
    #[arg(long, global = true)]
    pub include_sensitive: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Enumerate HID collections. Sends nothing and opens nothing for I/O.
    List(ListArgs),
    /// Read descriptors for one enumerated collection. Never writes.
    Inspect(InspectArgs),
    /// Read-only protocol probe. Performs no HID writes.
    Probe(ProbeArgs),
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Show only this vendor id, e.g. 0x1532.
    #[arg(long, value_parser = parse_u16_id)]
    pub vendor_id: Option<u16>,

    /// Show only this product id, e.g. 0x101b.
    #[arg(long, value_parser = parse_u16_id)]
    pub product_id: Option<u16>,
}

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Index from `headsetctl list`.
    #[arg(long)]
    pub path_index: usize,
}

#[derive(Args, Debug)]
pub struct ProbeArgs {
    /// Candidate index from `headsetctl list`. Defaults to the ranked best
    /// among the collections of the one device this project supports.
    #[arg(long)]
    pub candidate: Option<usize>,

    /// Restrict automatic candidate selection to this vendor id, e.g. 0x1532.
    /// Opts into a specific device in place of the built-in supported-device
    /// allowlist; has no effect when `--candidate` is also given.
    #[arg(long, value_parser = parse_u16_id)]
    pub vendor_id: Option<u16>,

    /// Restrict automatic candidate selection to this product id, e.g. 0x101b.
    /// Opts into a specific device in place of the built-in supported-device
    /// allowlist; has no effect when `--candidate` is also given.
    #[arg(long, value_parser = parse_u16_id)]
    pub product_id: Option<u16>,

    /// Milliseconds to listen for an unsolicited input report.
    #[arg(long, default_value_t = 2000)]
    pub listen_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_with_prefix() {
        assert_eq!(parse_u16_id("0x1532").unwrap(), 0x1532);
        assert_eq!(parse_u16_id("0X1532").unwrap(), 0x1532);
    }

    #[test]
    fn parses_bare_hex() {
        assert_eq!(parse_u16_id("1532").unwrap(), 0x1532);
        assert_eq!(parse_u16_id("101b").unwrap(), 0x101B);
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(parse_u16_id("0x10000").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_u16_id("zzzz").is_err());
        assert!(parse_u16_id("").is_err());
        assert!(parse_u16_id("-1").is_err());
    }
}

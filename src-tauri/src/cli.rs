//! Command-line argument parsing for the Tabularis binary.
//!
//! Keeping this in its own module means `lib.rs` does not have to know about
//! clap, and the public flag surface lives in one place.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Enable debug logging (including sqlx queries)
    #[arg(long)]
    pub debug: bool,

    /// Start without showing or activating the main window
    #[arg(long)]
    pub background: bool,

    /// Open a Visual Explain window for a previously-saved EXPLAIN file
    /// (Postgres `EXPLAIN (FORMAT JSON)` output).
    #[arg(long, value_name = "FILE")]
    pub explain: Option<String>,
}

impl Args {
    fn defaults() -> Self {
        Self {
            debug: false,
            background: false,
            explain: None,
        }
    }
}

/// Parse the process arguments, with platform-friendly fallback behaviour.
///
/// Help and version requests are printed by clap. Other parse failures fall
/// back to defaults so platform-specific GUI arguments still start the app.
pub fn parse() -> Args {
    Args::try_parse().unwrap_or_else(|error| {
        if matches!(
            error.kind(),
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
        ) {
            error.exit();
        }
        Args::defaults()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_flag_is_explicit_and_disabled_by_default() {
        assert!(!Args::defaults().background);
        let parsed = Args::try_parse_from(["tabularis", "--background"]).unwrap();
        assert!(parsed.background);
    }
}

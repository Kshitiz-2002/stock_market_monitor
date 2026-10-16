//! Command-line parsing without extra crates.
//!
//! `Config::build` takes any iterator of `String`s (normally
//! `std::env::args()`), which keeps it easy to unit test.

use crate::error::MonitorError;
use std::path::PathBuf;

/// Default volatility threshold: 2% of the closing price.
pub const DEFAULT_THRESHOLD_PCT: f64 = 2.0;

pub const HELP: &str = "\
stock_monitor - six-month closing price chart with volatile days highlighted

USAGE:
    stock_monitor <SYMBOL> [OPTIONS]

ARGS:
    <SYMBOL>    Stock ticker symbol, e.g. AAPL, MSFT, SHOP.TO, ^GSPC

OPTIONS:
    -o, --output <FILE>        Where to save the chart PNG
                               [default: output/<SYMBOL>_6mo.png]
    -t, --threshold <PERCENT>  Intraday range (high - low) as a percent of the
                               close above which a day counts as volatile
                               [default: 2]
    -h, --help                 Print this help and exit

EXAMPLE:
    cargo run --release -- AAPL
";

/// What the user asked the program to do.
#[derive(Debug, PartialEq)]
pub enum Command {
    ShowHelp,
    Monitor(Config),
}

/// Settings for one monitoring run.
#[derive(Debug, PartialEq)]
pub struct Config {
    pub symbol: String,
    pub output: PathBuf,
    pub threshold_pct: f64,
}

impl Config {
    /// Parse the arguments. The first item is the program name and is skipped.
    pub fn build(mut args: impl Iterator<Item = String>) -> Result<Command, MonitorError> {
        args.next();

        let mut symbol: Option<String> = None;
        let mut output: Option<PathBuf> = None;
        let mut threshold_pct = DEFAULT_THRESHOLD_PCT;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(Command::ShowHelp),
                "-o" | "--output" => {
                    let value = next_value(&mut args, &arg)?;
                    output = Some(PathBuf::from(value));
                }
                "-t" | "--threshold" => {
                    let value = next_value(&mut args, &arg)?;
                    threshold_pct = parse_threshold(&value)?;
                }
                flag if flag.starts_with('-') && flag.len() > 1 => {
                    return Err(MonitorError::Usage(format!("unknown option `{flag}`")));
                }
                _ if symbol.is_some() => {
                    return Err(MonitorError::Usage(format!(
                        "unexpected extra argument `{arg}` (only one symbol is allowed)"
                    )));
                }
                _ => symbol = Some(normalize_symbol(&arg)?),
            }
        }

        let symbol =
            symbol.ok_or_else(|| MonitorError::Usage("missing stock symbol".to_string()))?;
        let output = output.unwrap_or_else(|| default_output(&symbol));

        Ok(Command::Monitor(Config {
            symbol,
            output,
            threshold_pct,
        }))
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, MonitorError> {
    args.next()
        .ok_or_else(|| MonitorError::Usage(format!("`{flag}` needs a value")))
}

fn parse_threshold(value: &str) -> Result<f64, MonitorError> {
    value
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .filter(|pct| pct.is_finite() && *pct > 0.0 && *pct < 100.0)
        .ok_or_else(|| {
            MonitorError::Usage(format!(
                "threshold must be a number between 0 and 100, got `{value}`"
            ))
        })
}

/// Upper-case the ticker and reject characters that never appear in a
/// Yahoo symbol. This also stops odd input from being pasted into the URL
/// or the output file name.
fn normalize_symbol(raw: &str) -> Result<String, MonitorError> {
    let symbol = raw.trim().to_uppercase();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '=');

    if symbol.is_empty() || symbol.len() > 15 || !symbol.chars().all(allowed) {
        return Err(MonitorError::Usage(format!(
            "`{raw}` is not a valid ticker symbol"
        )));
    }
    Ok(symbol)
}

fn default_output(symbol: &str) -> PathBuf {
    // `^GSPC` and similar index tickers would give awkward file names.
    let file_stem: String = symbol
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    PathBuf::from("output").join(format!("{file_stem}_6mo.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Command, MonitorError> {
        Config::build(line.split_whitespace().map(String::from))
    }

    fn config(line: &str) -> Config {
        match parse(line) {
            Ok(Command::Monitor(config)) => config,
            other => panic!("expected a config, got {other:?}"),
        }
    }

    #[test]
    fn symbol_only_uses_defaults() {
        let c = config("prog aapl");
        assert_eq!(c.symbol, "AAPL");
        assert_eq!(c.output, PathBuf::from("output").join("AAPL_6mo.png"));
        assert_eq!(c.threshold_pct, DEFAULT_THRESHOLD_PCT);
    }

    #[test]
    fn help_wins_even_with_other_args() {
        assert_eq!(parse("prog --help").unwrap(), Command::ShowHelp);
        assert_eq!(parse("prog AAPL -h").unwrap(), Command::ShowHelp);
    }

    #[test]
    fn options_are_read() {
        let c = config("prog msft -t 3.5 --output chart.png");
        assert_eq!(c.symbol, "MSFT");
        assert_eq!(c.threshold_pct, 3.5);
        assert_eq!(c.output, PathBuf::from("chart.png"));
    }

    #[test]
    fn threshold_accepts_percent_sign() {
        assert_eq!(config("prog AAPL -t 1.5%").threshold_pct, 1.5);
    }

    #[test]
    fn index_symbol_gets_safe_file_name() {
        let c = config("prog ^gspc");
        assert_eq!(c.symbol, "^GSPC");
        assert_eq!(c.output, PathBuf::from("output").join("_GSPC_6mo.png"));
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        for line in [
            "prog",
            "prog AAPL MSFT",
            "prog AAPL --frobnicate",
            "prog AAPL -t",
            "prog AAPL -t abc",
            "prog AAPL -t -4",
            "prog AAPL -t 0",
            "prog AA$PL",
            "prog ../../etc",
        ] {
            assert!(
                matches!(parse(line), Err(MonitorError::Usage(_))),
                "`{line}` should be rejected"
            );
        }
    }
}

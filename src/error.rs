//! One error type for the whole program.
//!
//! Every failure (bad arguments, unknown ticker, network trouble, drawing
//! problems) is turned into a `MonitorError` and bubbled up with `?`, so
//! `main` is the only place that decides how to report it. Nothing panics.

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum MonitorError {
    /// The command line could not be understood.
    Usage(String),
    /// Yahoo Finance has no usable data for this ticker.
    UnknownSymbol(String),
    /// The request to Yahoo Finance failed for some other reason.
    Fetch(String),
    /// The chart could not be drawn or saved.
    Chart(String),
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorError::Usage(msg) => write!(f, "{msg}\nTry `stock_monitor --help` for usage."),
            MonitorError::UnknownSymbol(symbol) => write!(
                f,
                "no price data found for \"{symbol}\". Check that the ticker symbol is correct."
            ),
            MonitorError::Fetch(msg) => write!(f, "could not download quotes: {msg}"),
            MonitorError::Chart(msg) => write!(f, "could not draw the chart: {msg}"),
        }
    }
}

impl Error for MonitorError {}

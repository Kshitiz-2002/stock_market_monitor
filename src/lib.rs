//! Stock Market Monitor (ECE 522, Project 1).
//!
//! The program flow is: parse arguments (`cli`) -> download prices
//! (`quotes`) -> analyse them (`analysis`) -> draw the chart (`chart`) ->
//! print a short report. Each step returns a `Result`, so a bad symbol or a
//! network failure ends the run with a message instead of a crash.

pub mod analysis;
pub mod chart;
pub mod cli;
pub mod error;
pub mod quotes;

use analysis::Summary;
use cli::Config;
use error::MonitorError;
use quotes::PriceHistory;

pub fn run(config: &Config) -> Result<(), MonitorError> {
    println!(
        "Fetching six months of daily quotes for {} ...",
        config.symbol
    );
    let history = quotes::fetch_six_months(&config.symbol)?;

    let summary = analysis::summarize(&history.bars, config.threshold_pct)
        .ok_or_else(|| MonitorError::UnknownSymbol(config.symbol.clone()))?;

    chart::draw(&history, &summary, config.threshold_pct, &config.output)?;

    print!("{}", report(&history, &summary, config));
    Ok(())
}

/// The text printed after a successful run.
fn report(history: &PriceHistory, summary: &Summary, config: &Config) -> String {
    let currency = history.currency.as_deref().unwrap_or("");
    let first = &history.bars[0];
    let last = &history.bars[history.bars.len() - 1];

    let mut out = String::new();
    out += &format!(
        "\n{}{}\n",
        history.symbol,
        history
            .name
            .as_deref()
            .map(|n| format!(" - {n}"))
            .unwrap_or_default()
    );
    out += &format!(
        "Period: {} to {} ({} trading days)\n\n",
        first.date,
        last.date,
        history.bars.len()
    );
    out += &format!(
        "  Minimum close: {:>10.2} {currency} on {}\n",
        summary.min_close.close, summary.min_close.date
    );
    out += &format!(
        "  Maximum close: {:>10.2} {currency} on {}\n",
        summary.max_close.close, summary.max_close.date
    );
    out += &format!(
        "  Change over period: {:+.2}%\n\n",
        summary.period_change_pct()
    );

    out += &format!(
        "Volatile days (high - low > {}% of close): {}\n",
        config.threshold_pct,
        summary.volatile.len()
    );
    if !summary.volatile.is_empty() {
        out += "  Date          Low        High       Close      Range\n";
        out += &summary
            .volatile
            .iter()
            .map(|b| {
                format!(
                    "  {}  {:>9.2}  {:>9.2}  {:>9.2}  {:>6.2}%\n",
                    b.date,
                    b.low,
                    b.high,
                    b.close,
                    analysis::range_pct(b)
                )
            })
            .collect::<String>();
    }

    out += &format!("\nChart saved to {}\n", config.output.display());
    out
}

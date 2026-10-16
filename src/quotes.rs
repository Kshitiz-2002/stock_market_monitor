//! Downloading daily prices from Yahoo Finance.

use crate::error::MonitorError;
use yahoo_finance_api::time::{Date, OffsetDateTime, UtcOffset};
use yahoo_finance_api::{YahooConnector, YahooError};

/// One trading day of price data.
#[derive(Debug, Clone, PartialEq)]
pub struct DailyBar {
    pub date: Date,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// Everything the rest of the program needs about the downloaded series.
#[derive(Debug)]
pub struct PriceHistory {
    pub symbol: String,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub bars: Vec<DailyBar>,
}

/// Fetch the last six months of daily bars for `symbol`.
pub fn fetch_six_months(symbol: &str) -> Result<PriceHistory, MonitorError> {
    let provider = YahooConnector::new().map_err(|e| MonitorError::Fetch(e.to_string()))?;

    let response = provider
        .get_quote_range(symbol, "1d", "6mo")
        .map_err(|e| classify(symbol, e))?;

    let meta = response.metadata().map_err(|e| classify(symbol, e))?;
    let quotes = response.quotes().map_err(|e| classify(symbol, e))?;

    // Dates are taken in the exchange's own time zone, so a bar stamped at
    // 09:30 New York time is not shifted to the next day in UTC.
    let offset = UtcOffset::from_whole_seconds(meta.gmtoffset).unwrap_or(UtcOffset::UTC);

    let bars: Vec<DailyBar> = quotes
        .iter()
        .filter_map(|q| {
            let date = OffsetDateTime::from_unix_timestamp(q.timestamp)
                .ok()?
                .to_offset(offset)
                .date();
            let bar = DailyBar {
                date,
                open: q.open,
                high: q.high,
                low: q.low,
                close: q.close,
            };
            is_usable(&bar).then_some(bar)
        })
        .collect();

    if bars.is_empty() {
        return Err(MonitorError::UnknownSymbol(symbol.to_string()));
    }

    Ok(PriceHistory {
        symbol: meta.symbol,
        name: meta.long_name.or(meta.short_name),
        currency: meta.currency,
        bars,
    })
}

/// Yahoo sometimes sends half-empty rows (for example the current day while
/// the market is still open). Those would distort the chart, so drop them.
fn is_usable(bar: &DailyBar) -> bool {
    let prices = [bar.open, bar.high, bar.low, bar.close];
    prices.iter().all(|p| p.is_finite() && *p > 0.0) && bar.low <= bar.high
}

/// Turn the library's error into one of ours. Yahoo answers an unknown ticker
/// with HTTP 404, an API error ("No data found") or an empty result, and all
/// of those should read as "bad symbol" rather than as a network problem.
fn classify(symbol: &str, err: YahooError) -> MonitorError {
    match err {
        YahooError::FetchFailed(msg) if msg.starts_with("404") => {
            MonitorError::UnknownSymbol(symbol.to_string())
        }
        YahooError::ApiError(_)
        | YahooError::NoResult
        | YahooError::NoQuotes
        | YahooError::EmptyResponse
        | YahooError::DataInconsistency => MonitorError::UnknownSymbol(symbol.to_string()),
        other => MonitorError::Fetch(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yahoo_finance_api::time::Month;

    fn bar(open: f64, high: f64, low: f64, close: f64) -> DailyBar {
        let date = Date::from_calendar_date(2026, Month::March, 2).unwrap();
        DailyBar {
            date,
            open,
            high,
            low,
            close,
        }
    }

    #[test]
    fn normal_bar_is_usable() {
        assert!(is_usable(&bar(10.0, 11.0, 9.5, 10.5)));
    }

    #[test]
    fn not_found_means_unknown_symbol() {
        let err = YahooError::FetchFailed("404, request url: ...".to_string());
        assert!(matches!(
            classify("ZZZZ", err),
            MonitorError::UnknownSymbol(_)
        ));
        assert!(matches!(
            classify("ZZZZ", YahooError::NoQuotes),
            MonitorError::UnknownSymbol(_)
        ));

        let err = YahooError::FetchFailed("500 server trouble".to_string());
        assert!(matches!(classify("AAPL", err), MonitorError::Fetch(_)));
    }

    #[test]
    fn broken_bars_are_dropped() {
        assert!(!is_usable(&bar(0.0, 11.0, 9.5, 10.5)));
        assert!(!is_usable(&bar(10.0, 11.0, 9.5, f64::NAN)));
        assert!(!is_usable(&bar(10.0, 9.0, 9.5, 10.5))); // high below low
    }
}

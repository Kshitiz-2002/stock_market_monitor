//! The financial analysis: volatile days and the closing-price extremes.
//!
//! A day is *volatile* when its intraday range is large compared with the
//! price it closed at:
//!
//! ```text
//! range_pct = (high - low) / close * 100
//! volatile  = range_pct > threshold_pct      (threshold_pct = 2 by default)
//! ```

use crate::quotes::DailyBar;

/// Intraday high-low range as a percentage of the close.
pub fn range_pct(bar: &DailyBar) -> f64 {
    (bar.high - bar.low) / bar.close * 100.0
}

pub fn is_volatile(bar: &DailyBar, threshold_pct: f64) -> bool {
    range_pct(bar) > threshold_pct
}

/// Results of analysing one price series.
#[derive(Debug)]
pub struct Summary<'a> {
    pub min_close: &'a DailyBar,
    pub max_close: &'a DailyBar,
    pub volatile: Vec<&'a DailyBar>,
    pub first_close: f64,
    pub last_close: f64,
}

impl Summary<'_> {
    /// Change from the first to the last close in the window, in percent.
    pub fn period_change_pct(&self) -> f64 {
        (self.last_close - self.first_close) / self.first_close * 100.0
    }
}

/// Analyse the bars. Returns `None` only when there are no bars at all.
///
/// `f64` is not `Ord` (because of NaN), so `Iterator::max` cannot be used
/// directly; `max_by` with `total_cmp` gives the same functional style.
/// When the same extreme close appears more than once, the earliest date is
/// reported: `min_by` keeps the first minimum, while `max_by` keeps the last
/// maximum, so the max search runs over the reversed iterator.
pub fn summarize(bars: &[DailyBar], threshold_pct: f64) -> Option<Summary<'_>> {
    let min_close = bars.iter().min_by(|a, b| a.close.total_cmp(&b.close))?;
    let max_close = bars
        .iter()
        .rev()
        .max_by(|a, b| a.close.total_cmp(&b.close))?;

    let volatile = bars
        .iter()
        .filter(|bar| is_volatile(bar, threshold_pct))
        .collect();

    Some(Summary {
        min_close,
        max_close,
        volatile,
        first_close: bars.first()?.close,
        last_close: bars.last()?.close,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use yahoo_finance_api::time::{Date, Duration, Month};

    /// Build bars from (high, low, close) triples on consecutive days.
    fn bars(points: &[(f64, f64, f64)]) -> Vec<DailyBar> {
        let start = Date::from_calendar_date(2026, Month::April, 1).unwrap();
        points
            .iter()
            .enumerate()
            .map(|(i, &(high, low, close))| DailyBar {
                date: start + Duration::days(i as i64),
                open: close,
                high,
                low,
                close,
            })
            .collect()
    }

    #[test]
    fn range_is_relative_to_close() {
        let b = &bars(&[(102.0, 99.0, 100.0)])[0];
        assert!((range_pct(b) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn threshold_is_strictly_greater_than() {
        // Exactly 2% is *not* "more than 2%".
        let b = &bars(&[(101.0, 99.0, 100.0)])[0];
        assert!(!is_volatile(b, 2.0));
        assert!(is_volatile(b, 1.99));
    }

    #[test]
    fn finds_min_max_and_volatile_days() {
        let data = bars(&[
            (101.0, 100.0, 100.5), // 1.0%  calm
            (110.0, 104.0, 108.0), // 5.6%  volatile, max close
            (99.0, 98.5, 98.8),    // 0.5%  calm, min close
            (104.0, 100.0, 103.0), // 3.9%  volatile
        ]);
        let s = summarize(&data, 2.0).unwrap();

        assert_eq!(s.max_close.close, 108.0);
        assert_eq!(s.max_close.date, data[1].date);
        assert_eq!(s.min_close.close, 98.8);
        assert_eq!(s.min_close.date, data[2].date);

        let volatile_dates: Vec<_> = s.volatile.iter().map(|b| b.date).collect();
        assert_eq!(volatile_dates, vec![data[1].date, data[3].date]);

        assert!((s.period_change_pct() - 2.4875621).abs() < 1e-6);
    }

    #[test]
    fn ties_report_the_earliest_day() {
        let data = bars(&[
            (51.0, 49.0, 50.0),
            (61.0, 59.0, 60.0),
            (51.0, 49.0, 50.0),
            (61.0, 59.0, 60.0),
        ]);
        let s = summarize(&data, 2.0).unwrap();
        assert_eq!(s.min_close.date, data[0].date);
        assert_eq!(s.max_close.date, data[1].date);
    }

    #[test]
    fn empty_input_gives_none() {
        assert!(summarize(&[], 2.0).is_none());
    }
}

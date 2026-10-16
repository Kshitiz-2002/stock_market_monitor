//! Drawing the chart with `plotters`.
//!
//! Layout choices:
//! * The x axis counts *trading days* (0, 1, 2, ...) and the tick labels are
//!   the matching calendar dates. Weekends and holidays therefore leave no
//!   gaps in the line.
//! * The closing price is a line with a small dot per day.
//! * Volatile days get an error bar from the intraday low to the intraday
//!   high, with a tick at the close.
//! * The lowest and highest closes are marked and labelled.
//! * The y axis is scaled automatically to the data with a small margin.

use crate::analysis::{Summary, is_volatile};
use crate::error::MonitorError;
use crate::quotes::{DailyBar, PriceHistory};
use plotters::prelude::*;
use std::path::Path;
use yahoo_finance_api::time::Date;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

const CLOSE_COLOR: RGBColor = RGBColor(52, 73, 94);
const VOLATILE_COLOR: RGBColor = RGBColor(31, 119, 230);
const MAX_COLOR: RGBColor = RGBColor(39, 150, 80);
const MIN_COLOR: RGBColor = RGBColor(205, 55, 45);

pub fn draw(
    history: &PriceHistory,
    summary: &Summary,
    threshold_pct: f64,
    path: &Path,
) -> Result<(), MonitorError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| MonitorError::Chart(format!("cannot create {}: {e}", dir.display())))?;
    }
    // plotters reports errors with a generic type; flatten them to text.
    render(history, summary, threshold_pct, path).map_err(|e| MonitorError::Chart(e.to_string()))
}

fn render(
    history: &PriceHistory,
    summary: &Summary,
    threshold_pct: f64,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bars = &history.bars;
    let threshold = threshold_pct;

    // The visible price range: closes for every day, plus low/high for the
    // volatile days because their error bars are drawn too.
    let (y_low, y_high) = bars
        .iter()
        .map(|b| {
            if is_volatile(b, threshold) {
                (b.low, b.high)
            } else {
                (b.close, b.close)
            }
        })
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (l, h)| {
            (lo.min(l), hi.max(h))
        });
    let margin = ((y_high - y_low) * 0.08).max(y_high * 0.01);
    let y_range = (y_low - margin).max(0.0)..(y_high + margin);

    let last_index = bars.len() as i32 - 1;
    let x_range = -1..last_index + 2;

    let first = &bars[0].date;
    let last = &bars[bars.len() - 1].date;
    let title = match &history.name {
        Some(name) => format!("{} ({name}) - Daily Closing Price", history.symbol),
        None => format!("{} - Daily Closing Price", history.symbol),
    };
    let currency = history.currency.as_deref().unwrap_or("USD");

    let root = BitMapBackend::new(path, (WIDTH, HEIGHT)).into_drawing_area();
    root.fill(&WHITE)?;

    let (upper, lower) = root.split_vertically(HEIGHT - 28);
    lower.draw(&Text::new(
        format!(
            "{first} to {last}  |  {} trading days  |  {} volatile days (intraday range > {threshold}% of close)  |  source: Yahoo Finance",
            bars.len(),
            summary.volatile.len()
        ),
        (60, 6),
        ("sans-serif", 15).into_font().color(&RGBColor(90, 90, 90)),
    ))?;

    let mut chart = ChartBuilder::on(&upper)
        .caption(title, ("sans-serif", 28).into_font())
        .margin(16)
        .x_label_area_size(55)
        .y_label_area_size(80)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .x_desc("Date (trading days)")
        .y_desc(format!("Closing price ({currency})"))
        .axis_desc_style(("sans-serif", 18))
        .label_style(("sans-serif", 14))
        .x_labels(12)
        .y_labels(10)
        .x_label_formatter(&|x| label_for_index(bars, *x))
        .y_label_formatter(&|y| format!("{y:.2}"))
        .light_line_style(WHITE.mix(0.0))
        .bold_line_style(BLACK.mix(0.08))
        .draw()?;

    // Closing price line and per-day dots.
    chart
        .draw_series(LineSeries::new(
            bars.iter().enumerate().map(|(i, b)| (i as i32, b.close)),
            CLOSE_COLOR.stroke_width(2),
        ))?
        .label("Daily close")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], CLOSE_COLOR.stroke_width(2)));
    chart.draw_series(
        bars.iter()
            .enumerate()
            .map(|(i, b)| Circle::new((i as i32, b.close), 2, CLOSE_COLOR.filled())),
    )?;

    // Volatile days: low-close-high error bars.
    let index_of =
        |bar: &DailyBar| bars.iter().position(|b| b.date == bar.date).unwrap_or(0) as i32;
    chart
        .draw_series(summary.volatile.iter().map(|b| {
            ErrorBar::new_vertical(
                index_of(b),
                b.low,
                b.close,
                b.high,
                VOLATILE_COLOR.stroke_width(2),
                8,
            )
        }))?
        .label(format!(
            "Volatile day: low / close / high (range > {threshold}%)"
        ))
        .legend(|(x, y)| {
            PathElement::new(
                vec![(x + 10, y - 7), (x + 10, y + 7)],
                VOLATILE_COLOR.stroke_width(2),
            )
        });

    // Min and max closing prices.
    for (bar, color, word) in [
        (summary.max_close, MAX_COLOR, "Max"),
        (summary.min_close, MIN_COLOR, "Min"),
    ] {
        let x = index_of(bar);
        // Keep the label inside the plot: right of the point in the first
        // half of the chart, left of it in the second half.
        let dx = if x < last_index / 2 { 10 } else { -190 };
        let dy = if word == "Max" { -22 } else { 8 };
        chart
            .draw_series(std::iter::once(
                EmptyElement::at((x, bar.close))
                    + Circle::new((0, 0), 6, color.filled())
                    + Text::new(
                        format!("{word} {:.2} on {}", bar.close, bar.date),
                        (dx, dy),
                        ("sans-serif", 15).into_font().color(&color),
                    ),
            ))?
            .label(format!("{word} close"))
            .legend(move |(x, y)| Circle::new((x + 10, y), 5, color.filled()));
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .background_style(WHITE.mix(0.85))
        .border_style(BLACK.mix(0.3))
        .label_font(("sans-serif", 15))
        .draw()?;

    root.present()?;
    Ok(())
}

/// Tick label for a trading-day index, e.g. "Apr 14". Out-of-range indices
/// (the padding on either side) get no label.
fn label_for_index(bars: &[DailyBar], index: i32) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|i| bars.get(i))
        .map(|b| short_date(b.date))
        .unwrap_or_default()
}

fn short_date(date: Date) -> String {
    let month = date.month().to_string();
    format!("{} {}", &month[..3], date.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yahoo_finance_api::time::Month;

    #[test]
    fn short_date_uses_month_abbreviation() {
        let d = Date::from_calendar_date(2026, Month::September, 7).unwrap();
        assert_eq!(short_date(d), "Sep 7");
    }

    #[test]
    fn padding_indices_have_no_label() {
        let d = Date::from_calendar_date(2026, Month::May, 1).unwrap();
        let bars = vec![DailyBar {
            date: d,
            open: 1.0,
            high: 1.0,
            low: 1.0,
            close: 1.0,
        }];
        assert_eq!(label_for_index(&bars, 0), "May 1");
        assert_eq!(label_for_index(&bars, -1), "");
        assert_eq!(label_for_index(&bars, 1), "");
    }
}

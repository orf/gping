use anyhow::Context;
use chrono::prelude::*;
use core::option::Option;
use core::option::Option::{None, Some};
use core::time::Duration;
use itertools::Itertools;
use tui::style::Style;
use tui::symbols;
use tui::widgets::{Dataset, GraphType, Paragraph};

pub struct PlotData {
    pub display: String,
    pub data: Vec<(f64, f64)>,
    pub style: Style,
    buffer: chrono::Duration,
    simple_graphics: bool,
}

impl PlotData {
    pub fn new(display: String, buffer: u64, style: Style, simple_graphics: bool) -> PlotData {
        PlotData {
            display,
            data: Vec::with_capacity(150),
            style,
            buffer: chrono::Duration::try_seconds(buffer as i64)
                .with_context(|| format!("Error converting {buffer} to seconds"))
                .unwrap(),
            simple_graphics,
        }
    }
    pub fn update(&mut self, item: Option<Duration>) {
        let now = Local::now();
        let idx = now.timestamp_millis() as f64 / 1_000f64;
        match item {
            Some(dur) => self.data.push((idx, dur.as_micros() as f64)),
            None => self.data.push((idx, f64::NAN)),
        }
        // Find the last index that we should remove.
        let earliest_timestamp = (now - self.buffer).timestamp_millis() as f64 / 1_000f64;
        let last_idx = self
            .data
            .iter()
            .enumerate()
            .filter(|(_, (timestamp, _))| *timestamp < earliest_timestamp)
            .map(|(idx, _)| idx)
            .next_back();
        if let Some(idx) = last_idx {
            // `idx` itself is still stale (it matched the filter above), so it must be
            // included in the drained range too, otherwise one out-of-window point is
            // always left behind.
            self.data.drain(0..=idx).for_each(drop)
        }
    }

    pub fn header_stats(&self) -> Vec<Paragraph<'_>> {
        let ping_header = Paragraph::new(self.display.clone()).style(self.style);
        // Chronologically-ordered (i.e. not sorted) latencies, used for the jitter
        // calculation which needs to compare consecutive samples in the order they
        // actually occurred.
        let chronological: Vec<f64> = self
            .data
            .iter()
            .filter(|(_, x)| !x.is_nan())
            .map(|(_, v)| *v)
            .collect();
        let items: Vec<&f64> = self
            .data
            .iter()
            .filter(|(_, x)| !x.is_nan())
            .map(|(_, v)| v)
            .sorted_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .collect();
        if items.is_empty() {
            return vec![ping_header];
        }

        let min = **items.first().unwrap();
        let max = **items.last().unwrap();
        let avg = items.iter().copied().sum::<f64>() / items.len() as f64;
        let jtr = jitter(&chronological);

        let percentile_position = 0.95 * items.len() as f32;
        let rounded_position = percentile_position.round() as usize;
        let p95 = items.get(rounded_position).map(|i| **i).unwrap_or(0f64);

        // count timeouts
        let to = self.data.iter().filter(|(_, x)| x.is_nan()).count();

        let last = self.data.last().unwrap_or(&(0f64, 0f64)).1;

        vec![
            ping_header,
            Paragraph::new(format!("last {:?}", Duration::from_micros(last as u64)))
                .style(self.style),
            Paragraph::new(format!("min {:?}", Duration::from_micros(min as u64)))
                .style(self.style),
            Paragraph::new(format!("max {:?}", Duration::from_micros(max as u64)))
                .style(self.style),
            Paragraph::new(format!("avg {:?}", Duration::from_micros(avg as u64)))
                .style(self.style),
            Paragraph::new(format!("jtr {:?}", Duration::from_micros(jtr as u64)))
                .style(self.style),
            Paragraph::new(format!("p95 {:?}", Duration::from_micros(p95 as u64)))
                .style(self.style),
            Paragraph::new(format!("t/o {to:?}")).style(self.style),
        ]
    }
}

/// Average absolute difference between consecutive values, taken in the order
/// they are given (i.e. the caller is responsible for passing them in
/// chronological order, not sorted). With fewer than two values there is no
/// consecutive pair to compare, so jitter is reported as 0.
fn jitter(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    values
        .iter()
        .zip(values.iter().skip(1))
        .map(|(prev, curr)| (curr - prev).abs())
        .sum::<f64>()
        / (values.len() - 1) as f64
}

impl<'a> From<&'a PlotData> for Dataset<'a> {
    fn from(plot: &'a PlotData) -> Self {
        let slice = plot.data.as_slice();
        Dataset::default()
            .marker(if plot.simple_graphics {
                symbols::Marker::Dot
            } else {
                symbols::Marker::Braille
            })
            .style(plot.style)
            .graph_type(GraphType::Line)
            .data(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression test for the buffer trim in `PlotData::update`: every point older than
    // `buffer` seconds should be dropped, including the single oldest stale point, which
    // used to survive because `drain(0..idx)` excluded the boundary index itself.
    #[test]
    fn update_drops_all_points_outside_the_buffer_window() {
        let buffer_secs = 5u64;
        let mut plot = PlotData::new("host".to_string(), buffer_secs, Style::default(), false);

        let now = Local::now().timestamp_millis() as f64 / 1_000f64;

        // Seed with a mix of stale (older than the buffer window) and fresh points,
        // pushed in ascending timestamp order like the real update loop would.
        plot.data.push((now - 10.0, 100.0)); // stale
        plot.data.push((now - 8.0, 200.0)); // stale
        plot.data.push((now - 6.0, 300.0)); // stale, closest to the boundary
        plot.data.push((now - 2.0, 400.0)); // fresh
        plot.data.push((now - 1.0, 500.0)); // fresh

        // Triggers the trim logic; also appends one brand-new point.
        plot.update(Some(Duration::from_millis(42)));

        let earliest_allowed = now - buffer_secs as f64;
        for &(timestamp, _) in &plot.data {
            assert!(
                timestamp >= earliest_allowed,
                "found a point at {timestamp}, which is older than the buffer window start {earliest_allowed}; data: {:?}",
                plot.data
            );
        }
    }

    #[test]
    fn test_jitter_uses_chronological_order() {
        // Oscillating latencies: sorted-order "jitter" would telescope down to
        // (max - min) / (n - 1) = (90 - 10) / 5 = 16, which is really a range
        // statistic, not jitter. In chronological order every consecutive pair
        // differs by 80, so the real jitter should be 80.
        let values = vec![10.0, 90.0, 10.0, 90.0, 10.0, 90.0];

        let sorted_order_value = (90.0 - 10.0) / (values.len() - 1) as f64;
        let result = jitter(&values);

        assert_eq!(result, 80.0);
        assert_ne!(result, sorted_order_value);
    }

    #[test]
    fn test_jitter_single_sample_is_zero() {
        assert_eq!(jitter(&[42.0]), 0.0);
    }

    #[test]
    fn test_jitter_empty_is_zero() {
        assert_eq!(jitter(&[]), 0.0);
    }
}

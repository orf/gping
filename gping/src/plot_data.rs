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
    /// Total packets sent so far (never trimmed with the graph buffer).
    sent: u64,
    /// Total packets that timed out so far (never trimmed either).
    timeouts: u64,
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
            sent: 0,
            timeouts: 0,
        }
    }
    pub fn update(&mut self, item: Option<Duration>) {
        let now = Local::now();
        let idx = now.timestamp_millis() as f64 / 1_000f64;
        self.sent += 1;
        match item {
            Some(dur) => self.data.push((idx, dur.as_micros() as f64)),
            None => {
                self.timeouts += 1;
                self.data.push((idx, f64::NAN));
            }
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

    /// Packet loss over the whole session (not just the graph buffer window):
    /// returns `(loss_percent, timeouts, sent)`.
    fn loss_stats(&self) -> (f64, u64, u64) {
        let loss_percent = if self.sent > 0 {
            self.timeouts as f64 / self.sent as f64 * 100.0
        } else {
            0.0
        };
        (loss_percent, self.timeouts, self.sent)
    }

    pub fn header_stats(&self) -> Vec<Paragraph<'_>> {
        let ping_header = Paragraph::new(self.display.clone()).style(self.style);
        let (loss_percent, timeouts_total, sent_total) = self.loss_stats();
        let loss_line = Paragraph::new(format_loss_line(loss_percent, timeouts_total, sent_total))
            .style(self.style);
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
            // No successful ping in the current buffer window (e.g. at startup, or
            // if every packet timed out): still show the packet loss rate.
            return vec![ping_header, loss_line];
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
            loss_line,
        ]
    }
}

/// Render the packet loss line shown in the header stats, e.g. `loss 19.0% (19/100)`.
fn format_loss_line(loss_percent: f64, timeouts: u64, sent: u64) -> String {
    format!("loss {loss_percent:.1}% ({timeouts}/{sent})")
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

    // --- Packet loss rate (#114) ---

    fn make_plot() -> PlotData {
        PlotData::new("host".to_string(), 30, Style::default(), false)
    }

    #[test]
    fn test_loss_stats_start_at_zero() {
        let plot = make_plot();
        assert_eq!(plot.loss_stats(), (0.0, 0, 0));
        assert_eq!(format_loss_line(0.0, 0, 0), "loss 0.0% (0/0)");
    }

    #[test]
    fn test_update_counts_sent_and_timeouts() {
        let mut plot = make_plot();
        plot.update(Some(Duration::from_millis(10)));
        plot.update(Some(Duration::from_millis(20)));
        plot.update(None); // timeout
        plot.update(Some(Duration::from_millis(30)));
        plot.update(None); // timeout

        let (loss_percent, timeouts, sent) = plot.loss_stats();
        assert_eq!(sent, 5);
        assert_eq!(timeouts, 2);
        assert!((loss_percent - 40.0).abs() < 1e-9);
        assert_eq!(
            format_loss_line(loss_percent, timeouts, sent),
            "loss 40.0% (2/5)"
        );
    }

    #[test]
    fn test_loss_counters_survive_buffer_trim() {
        // The graph buffer trims old points, but the loss counters must be
        // cumulative over the whole session, like `ping`'s summary line.
        let mut plot = PlotData::new("host".to_string(), 1, Style::default(), false); // 1s buffer
        let now = Local::now().timestamp_millis() as f64 / 1_000f64;
        // Seed stale points directly, then update: the stale ones get trimmed.
        plot.data.push((now - 100.0, 100.0));
        plot.data.push((now - 90.0, f64::NAN));
        plot.update(Some(Duration::from_millis(42)));

        let (loss_percent, timeouts, sent) = plot.loss_stats();
        // `update` was called once, so sent == 1 even though the seeded points
        // were trimmed from the graph data.
        assert_eq!(sent, 1);
        assert_eq!(timeouts, 0);
        assert_eq!(loss_percent, 0.0);
        assert!(
            plot.data.iter().all(|(ts, _)| *ts >= now - 1.0),
            "stale points should have been trimmed: {:?}",
            plot.data
        );
    }

    #[test]
    fn test_loss_counters_count_seeded_updates() {
        // Drive the counters purely through `update` with a tiny buffer so the
        // graph window empties out, while the counters keep counting.
        let mut plot = PlotData::new("host".to_string(), 0, Style::default(), false); // 0s buffer trims everything old
        for _ in 0..3 {
            plot.update(None);
        }
        plot.update(Some(Duration::from_millis(5)));

        let (loss_percent, timeouts, sent) = plot.loss_stats();
        assert_eq!(sent, 4);
        assert_eq!(timeouts, 3);
        assert!((loss_percent - 75.0).abs() < 1e-9);
    }

    #[test]
    fn test_header_stats_includes_loss_line() {
        let mut plot = make_plot();
        plot.update(Some(Duration::from_millis(10)));
        plot.update(None);

        let header = plot.header_stats();
        // host line + 7 stat lines + loss line
        assert_eq!(header.len(), 9);
    }

    #[test]
    fn test_header_stats_all_timeouts_still_shows_loss() {
        // Every packet timed out: no non-NaN items, but the loss line must
        // still be rendered (with 100% loss).
        let mut plot = make_plot();
        plot.update(None);
        plot.update(None);

        let header = plot.header_stats();
        assert_eq!(header.len(), 2); // host line + loss line only
        let (loss_percent, timeouts, sent) = plot.loss_stats();
        assert_eq!((timeouts, sent), (2, 2));
        assert!((loss_percent - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_format_loss_line_rounds_to_one_decimal() {
        assert_eq!(format_loss_line(33.333, 1, 3), "loss 33.3% (1/3)");
        assert_eq!(format_loss_line(0.0, 0, 10), "loss 0.0% (0/10)");
    }
}

use crate::ringbuffer;
use core::option::Option;
use core::option::Option::{None, Some};
use core::time::Duration;
use histogram::Histogram;
use tui::style::Style;
use tui::symbols;
use tui::widgets::{Dataset, GraphType, Paragraph};

pub struct PlotData {
    pub display: String,
    pub data: ringbuffer::FixedRingBuffer<(f64, f64)>,
    pub idx: i64,
    pub window_min: f64,
    pub window_max: f64,
    pub style: Style,
}

impl PlotData {
    pub fn new(display: String, capacity: usize, style: Style) -> PlotData {
        PlotData {
            display,
            data: ringbuffer::FixedRingBuffer::new(capacity),
            idx: 0,
            window_min: 0.0,
            window_max: capacity as f64,
            style,
        }
    }
    pub fn update(&mut self, item: Option<Duration>) {
        self.idx += 1;
        if self.data.len() >= self.data.cap {
            self.window_min += 1_f64;
            self.window_max += 1_f64;
        }
        match item {
            Some(dur) => self.data.push((self.idx as f64, dur.as_micros() as f64)),
            None => self.data.push((self.idx as f64, 0_f64)),
        }
    }

    pub fn stats(&self) -> Histogram {
        let mut hist = Histogram::new();

        for (_, val) in self.data.iter().filter(|v| v.1 != 0f64) {
            hist.increment(*val as u64).unwrap_or(());
        }

        hist
    }

    pub fn header_stats(&self) -> Vec<Paragraph> {
        let stats = self.stats();
        vec![
            Paragraph::new(self.display.clone()).style(self.style),
            Paragraph::new(format!(
                "min {:?}",
                Duration::from_micros(stats.minimum().unwrap_or(0))
            ))
            .style(self.style),
            Paragraph::new(format!(
                "max {:?}",
                Duration::from_micros(stats.maximum().unwrap_or(0))
            ))
            .style(self.style),
            Paragraph::new(format!(
                "p95 {:?}",
                Duration::from_micros(stats.percentile(95.0).unwrap_or(0))
            ))
            .style(self.style),
        ]
    }
}

impl<'a> Into<Dataset<'a>> for &'a PlotData {
    fn into(self) -> Dataset<'a> {
        let slice = self.data.as_slice();
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .style(self.style)
            .graph_type(GraphType::Line)
            .data(slice)
        // .x_axis_bounds([self.window_min, self.window_max])
    }
}

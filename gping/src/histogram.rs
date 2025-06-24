use core::time::Duration;

use tui::{
    buffer::Buffer,
    layout::{Layout, Constraint, Flex, Rect},
    symbols,
    style::{Color, Style},
    text::Line,
    widgets::{Axis, Block, Chart, Dataset, GraphType, Padding, Paragraph, Widget},
};

/// defines the x-axis extent, effectively a zoom parameter
const OVERFLOW_SIZE : usize = 15;

const DEFAULT_WINDOW_SIZE : usize = 500;

#[derive(Debug)]
pub struct HistogramState {
    pub enabled: bool,
    /// the raw data used to compute the histogram
    /// the length of this cannot exceed window_size, if set
    pub samples : Vec<u64>,
    /// how many samples to use when generating the historgram
    /// if None, all samples will be used without limit.
    pub window_size : Option<usize>,
    pub bin_buckets : Vec<u64>,
    pub bin_counts : Vec<u64>,
    plot_data: Vec<(f64, f64)>,
    max_count: u64,
    max_bin: u64,
    overflow_bin: u64
}

impl Default for HistogramState {
    fn default() -> Self {
        let bin_buckets: Vec<u64> = [
            ( 1   .. 50   ).step_by(1).collect::<Vec<i64>>(), 
            ( 50  .. 250   ).step_by(5).collect::<Vec<i64>>(),
            ( 250 .. 1000 ).step_by(100).collect::<Vec<i64>>(),
        ].concat().iter().map(|x| *x as u64).collect();

        HistogramState { 
            samples: Vec::new(), 
            window_size: Some(DEFAULT_WINDOW_SIZE), 
            bin_counts: vec![0; bin_buckets.len()],
            plot_data: Vec::new(),
            overflow_bin: bin_buckets[bin_buckets.len() - 1],
            max_bin: 0,
            max_count: 0,
            enabled: false,
            bin_buckets,
        }
    }
}


/// helper function to create a top-right rect using up certain percentage of the available rect `r`
/// modified from https://ratatui.rs/examples/apps/popup/
fn popup_area(area: Rect, consume_y_pct: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(consume_y_pct)]).flex(Flex::Start);
    let horizontal = Layout::horizontal([Constraint::Fill(1), Constraint::Min(20)]);
    let [area] = vertical.areas(area);
    let [_, area] = horizontal.areas(area);
    area
}

impl HistogramState {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn add_sample(&mut self, x: Option<Duration>) {
        let x = match x {
            None => u64::MAX,
            Some(d ) => {
                let millis = d.as_millis();
                if millis >= u64::MAX as u128 {
                    u64::MAX
                } else {
                    millis as u64
                }
            }            
        };

        self.samples.push(x);
        
        // roll window
        if let Some(window) = self.window_size {
            while self.samples.len() > window {
                self.samples.remove(0);
            }
        }

        // we collect data when disabled, but we don't do anything else.
        if self.enabled {
            self.update();
        }
    }

    fn _bin_index(&self, x: &u64) -> usize {
        for i in 0 .. self.bin_buckets.len() {
            if *x <= self.bin_buckets[i] {
                return i
            }
        }

        self.bin_buckets.len() - 1
    }    

    // FIXME: not efficient, recalculates from scratch 
    fn update_bins(&mut self) {
        // initialize
        let n = self.bin_counts.len();

        self.bin_counts = Vec::with_capacity(n);
        self.bin_counts.resize(n, 0);
        
        // count
        for i in self.samples.iter() {
            let idx = self._bin_index(i);
            self.bin_counts[idx] += 1
        }
    }

    fn update(&mut self) {
        self.update_bins();

        self.max_count = *self.bin_counts.iter().max().unwrap_or(&0);        
        let (max_bin, overflow_bin_idx) = {
            let max_bin_idx: usize = self.bin_counts.iter().position(|&x| x == self.max_count).unwrap_or(self.bin_buckets.len() - 1);

            let next_max_bin = match max_bin_idx {
                x if x + OVERFLOW_SIZE >= self.bin_buckets.len() - 1 => self.bin_buckets.len() - 1,
                x => x + OVERFLOW_SIZE
            };

            (self.bin_buckets[max_bin_idx], next_max_bin)
        };

        self.max_bin = max_bin;
        self.overflow_bin = self.bin_buckets[overflow_bin_idx];

        let overflow: u64 = self.bin_counts[overflow_bin_idx..].iter().sum();

        let mut plot_data: Vec<(f64, f64)> = self.bin_buckets.iter().map(|x| *x as f64).zip(self.bin_counts.iter().map(|x| *x as f64)).collect();

        // add the overflow to the last visible bin
        if overflow > 0 {
            plot_data.get_mut(overflow_bin_idx).unwrap().1 += overflow as f64
        }

        self.plot_data = plot_data;
    }

    fn dataset(&self) -> Dataset<'_> {
        Dataset::default()
            .marker(symbols::Marker::HalfBlock)
            .style(Style::new().fg(Color::White))
            .graph_type(GraphType::Bar)
            .data(&self.plot_data) 
    }

    pub fn render_histogram(&self, area: &Rect, buffer: &mut Buffer) {
        let dataset = self.dataset();

        Chart::new(vec![dataset])
            .block(Block::new().padding(Padding{ left: 2, right: 2, top: 2, bottom: 2}))
            .x_axis(
                Axis::default()
                .bounds([0.0, self.overflow_bin as f64])
            )
            .y_axis(
                Axis::default()
                    .bounds([0.0, self.max_count as f64])
            )
            .render(*area, buffer);  

        let stats_area = popup_area(*area, 25);
        let stats_text = vec![
            Line::from(vec![
                format!("Samples: {}", self.samples.len()).into(),
            ]),
            Line::from(vec![
                format!("Mode: {} ms", self.max_bin).into(),
            ]),
            Line::from(vec![
                format!("Overflow >= {} ms", self.overflow_bin).into(),
            ])            
        ];
        Paragraph::new(stats_text).block(Block::bordered().title("Stats")).render(stats_area, buffer);
    }
}
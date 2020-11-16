mod ringbuffer;

use anyhow::Result;
use crossterm::event::{KeyEvent, KeyModifiers};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use histogram::Histogram;
use pinger::{ping, PingResult};
use std::io;
use std::io::Write;
use std::ops::Add;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};
use tui::{symbols, Terminal};

#[derive(Debug, StructOpt)]
#[structopt(name = "gping", about = "Ping, but with a graph.")]
struct Args {
    #[structopt(help = "Host or IP to ping")]
    host: String,
    #[structopt(
        short,
        long,
        default_value = "100",
        help = "Determines the number pings to display."
    )]
    buffer: usize,
}

struct App {
    data: ringbuffer::FixedRingBuffer<(f64, f64)>,
    capacity: usize,
    idx: i64,
    window: [f64; 2],
}

impl App {
    fn new(capacity: usize) -> Self {
        App {
            data: ringbuffer::FixedRingBuffer::new(capacity),
            capacity,
            idx: 0,
            window: [0.0, capacity as f64],
        }
    }
    fn update(&mut self, item: Option<Duration>) {
        self.idx += 1;
        if self.data.len() >= self.capacity {
            self.window[0] += 1_f64;
            self.window[1] += 1_f64;
        }
        match item {
            Some(dur) => self.data.push((self.idx as f64, dur.as_micros() as f64)),
            None => self.data.push((self.idx as f64, 0_f64)),
        }
    }
    fn stats(&self) -> Histogram {
        let mut hist = Histogram::new();

        for (_, val) in self.data.iter().filter(|v| v.1 != 0f64) {
            hist.increment(*val as u64).unwrap_or(());
        }

        hist
    }
    fn y_axis_bounds(&self) -> [f64; 2] {
        let min = self
            .data
            .iter()
            .map(|v| v.1)
            .fold(f64::INFINITY, |a, b| a.min(b));
        let max = self.data.iter().map(|v| v.1).fold(0f64, |a, b| a.max(b));
        // Add a 10% buffer to the top and bottom
        let max_10_percent = (max * 10_f64) / 100_f64;
        let min_10_percent = (min * 10_f64) / 100_f64;
        [min - min_10_percent, max + max_10_percent]
    }
    fn y_axis_labels(&self, bounds: [f64; 2]) -> Vec<Span> {
        // Split into 5 sections
        let min = bounds[0];
        let max = bounds[1];

        let difference = max - min;
        let increment = Duration::from_micros((difference / 3f64) as u64);
        let duration = Duration::from_micros(min as u64);

        (0..7)
            .map(|i| Span::raw(format!("{:?}", duration.add(increment * i))))
            .collect()
    }
}

#[derive(Debug)]
enum Event {
    Update(PingResult),
    Input(KeyEvent),
}

fn main() -> Result<()> {
    let args = Args::from_args();
    let mut app = App::new(args.buffer);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let (key_tx, rx) = mpsc::channel();

    let ping_tx = key_tx.clone();

    let killed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let host = args.host.clone();

    // Pump ping messages into the queue
    let killed_ping = std::sync::Arc::clone(&killed);
    let ping_thread = thread::spawn(move || -> Result<()> {
        let stream = ping(host)?;
        while !killed_ping.load(Ordering::Acquire) {
            ping_tx.send(Event::Update(stream.recv()?))?;
        }
        Ok(())
    });

    // Pump keyboard messages into the queue
    let killed_thread = std::sync::Arc::clone(&killed);
    let key_thread = thread::spawn(move || -> Result<()> {
        while !killed_thread.load(Ordering::Acquire) {
            if event::poll(Duration::from_secs(1))? {
                if let CEvent::Key(key) = event::read()? {
                    key_tx.send(Event::Input(key))?;
                }
            }
        }
        Ok(())
    });

    loop {
        match rx.recv()? {
            Event::Update(ping_result) => {
                match ping_result {
                    PingResult::Pong(duration) => app.update(Some(duration)),
                    PingResult::Timeout => app.update(None),
                };
                terminal.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(2)
                        .constraints(
                            [Constraint::Percentage(5), Constraint::Percentage(10)].as_ref(),
                        )
                        .split(f.size());

                    let header_layout = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [
                                Constraint::Percentage(25),
                                Constraint::Percentage(25),
                                Constraint::Percentage(25),
                                Constraint::Percentage(25),
                            ]
                            .as_ref(),
                        )
                        .split(chunks[0]);

                    f.render_widget(
                        Paragraph::new(format!("Pinging {}", &args.host)),
                        header_layout[0],
                    );

                    let stats = app.stats();

                    f.render_widget(
                        Paragraph::new(format!(
                            "min {:?}",
                            Duration::from_micros(stats.minimum().unwrap_or(0))
                        )),
                        header_layout[1],
                    );
                    f.render_widget(
                        Paragraph::new(format!(
                            "max {:?}",
                            Duration::from_micros(stats.maximum().unwrap_or(0))
                        )),
                        header_layout[2],
                    );
                    f.render_widget(
                        Paragraph::new(format!(
                            "p95 {:?}",
                            Duration::from_micros(stats.percentile(95.0).unwrap_or(0))
                        )),
                        header_layout[3],
                    );

                    let dataset = Dataset::default()
                        .marker(symbols::Marker::Braille)
                        .style(Style::default().fg(Color::Cyan))
                        .graph_type(GraphType::Line)
                        .data(&app.data.as_slice());

                    let y_axis_bounds = app.y_axis_bounds();

                    let chart = Chart::new(vec![dataset])
                        .block(Block::default().borders(Borders::NONE))
                        .x_axis(
                            Axis::default()
                                .style(Style::default().fg(Color::Gray))
                                .bounds(app.window),
                        )
                        .y_axis(
                            Axis::default()
                                .style(Style::default().fg(Color::Gray))
                                .bounds(y_axis_bounds)
                                .labels(app.y_axis_labels(y_axis_bounds)),
                        );
                    f.render_widget(chart, chunks[1]);
                })?;
            }
            Event::Input(input) => match input.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    killed.store(true, Ordering::Release);
                    break;
                }
                KeyCode::Char('c') if input.modifiers == KeyModifiers::CONTROL => {
                    killed.store(true, Ordering::Release);
                    break;
                }
                _ => {}
            },
        }
    }

    let _ = ping_thread.join().unwrap()?;
    let _ = key_thread.join().unwrap()?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

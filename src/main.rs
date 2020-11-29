mod ringbuffer;

use crate::plot_data::PlotData;
use anyhow::{anyhow, Result};
use crossterm::event::{KeyEvent, KeyModifiers};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dns_lookup::lookup_host;
use pinger::{ping, PingResult};
use std::io;
use std::io::Write;
use std::iter;
use std::net::IpAddr;
use std::ops::Add;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::{Axis, Block, Borders, Chart, Dataset};
use tui::Terminal;

mod plot_data;

#[derive(Debug, StructOpt)]
#[structopt(name = "gping", about = "Ping, but with a graph.")]
struct Args {
    #[structopt(
        long,
        help = "Graph the execution time for a list of commands rather than pinging hosts"
    )]
    cmd: bool,
    #[structopt(
        short = "n",
        long,
        help = "Watch interval seconds (provide partial seconds like '0.5')",
        default_value = "0.5"
    )]
    watch_interval: f32,
    #[structopt(help = "Hosts or IPs to ping, or commands to run if --cmd is provided.")]
    hosts_or_commands: Vec<String>,
    #[structopt(
        short,
        long,
        default_value = "100",
        help = "Determines the number pings to display."
    )]
    buffer: usize,
}

struct App {
    data: Vec<PlotData>,
}

impl App {
    fn new(data: Vec<PlotData>) -> Self {
        App { data }
    }

    fn update(&mut self, host_idx: usize, item: Option<Duration>) {
        let host = &mut self.data[host_idx];
        host.update(item);
    }

    fn y_axis_bounds(&self) -> [f64; 2] {
        // Find the Y axis bounds for our chart.
        // This is trickier than the x-axis. We iterate through all our PlotData structs
        // and find the min/max of all the values. Then we add a 10% buffer to them.
        let iter = self
            .data
            .iter()
            .map(|b| b.data.as_slice())
            .flatten()
            .map(|v| v.1);
        let min = iter.clone().fold(f64::INFINITY, |a, b| a.min(b));
        let max = iter.fold(0f64, |a, b| a.max(b));
        // Add a 10% buffer to the top and bottom
        let max_10_percent = (max * 10_f64) / 100_f64;
        let min_10_percent = (min * 10_f64) / 100_f64;
        [min - min_10_percent, max + max_10_percent]
    }

    fn x_axis_bounds(&self) -> [f64; 2] {
        let window_min = self.data.iter().map(|d| d.window_min);
        let window_max = self.data.iter().map(|d| d.window_max);
        [
            window_min.fold(f64::INFINITY, |a, b| a.min(b)),
            window_max.fold(0f64, |a, b| a.max(b)),
        ]
    }

    fn y_axis_labels(&self, bounds: [f64; 2]) -> Vec<Span> {
        // Create 7 labels for our y axis, based on the y-axis bounds we computed above.
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
enum Update {
    Result(Duration),
    Timeout,
    Unknown,
}

impl From<PingResult> for Update {
    fn from(result: PingResult) -> Self {
        match result {
            PingResult::Pong(duration, _) => Update::Result(duration),
            PingResult::Timeout(_) => Update::Timeout,
            PingResult::Unknown(_) => Update::Unknown,
        }
    }
}

#[derive(Debug)]
enum Event {
    Update(usize, Update),
    Input(KeyEvent),
}

fn start_cmd_thread(
    watch_cmd: &str,
    host_id: usize,
    watch_interval: f32,
    cmd_tx: Sender<Event>,
    kill_event: Arc<AtomicBool>,
) -> JoinHandle<Result<()>> {
    let mut words = watch_cmd.split_ascii_whitespace();
    let cmd = words
        .next()
        .expect("Must specify a command to watch")
        .to_string();
    let cmd_args = words
        .into_iter()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();

    let interval = Duration::from_millis((watch_interval * 1000.0) as u64);

    // Pump cmd watches into the queue
    thread::spawn(move || -> Result<()> {
        while !kill_event.load(Ordering::Acquire) {
            let start = Instant::now();
            let mut child = Command::new(&cmd)
                .args(&cmd_args)
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .spawn()?;
            let status = child.wait()?;
            let duration = start.elapsed();
            let update = if status.success() {
                Update::Result(duration)
            } else {
                Update::Timeout
            };
            cmd_tx.send(Event::Update(host_id, update))?;
            thread::sleep(interval);
        }
        Ok(())
    })
}

fn start_ping_thread(
    host: String,
    host_id: usize,
    ping_tx: Sender<Event>,
    kill_event: Arc<AtomicBool>,
) -> JoinHandle<Result<()>> {
    // Pump ping messages into the queue
    thread::spawn(move || -> Result<()> {
        let stream = ping(host)?;
        while !kill_event.load(Ordering::Acquire) {
            ping_tx.send(Event::Update(host_id, stream.recv()?.into()))?;
        }
        Ok(())
    })
}

fn get_host_ipaddr(host: &str) -> Result<String> {
    let ipaddr: Vec<IpAddr> = match lookup_host(host) {
        Ok(ip) => ip,
        Err(_) => return Err(anyhow!("Could not resolve hostname {}", host)),
    };
    let ipaddr = ipaddr.first();
    Ok(ipaddr.unwrap().to_string())
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let mut data = vec![];

    for (idx, host_or_cmd) in args.hosts_or_commands.iter().enumerate() {
        let display = match args.cmd {
            true => host_or_cmd.to_string(),
            false => format!("{} ({})", host_or_cmd, get_host_ipaddr(host_or_cmd)?),
        };
        data.push(PlotData::new(
            display,
            args.buffer,
            Style::default().fg(Color::Indexed(idx as u8 + 1)),
        ));
    }

    let mut app = App::new(data);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let (key_tx, rx) = mpsc::channel();

    let mut threads = vec![];

    let killed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    for (host_id, host_or_cmd) in args.hosts_or_commands.iter().cloned().enumerate() {
        if args.cmd {
            let cmd_thread = start_cmd_thread(
                &host_or_cmd,
                host_id,
                args.watch_interval,
                key_tx.clone(),
                std::sync::Arc::clone(&killed),
            );
            threads.push(cmd_thread);
        } else {
            threads.push(start_ping_thread(
                host_or_cmd,
                host_id,
                key_tx.clone(),
                std::sync::Arc::clone(&killed),
            ));
        }
    }

    // Pump keyboard messages into the queue
    let killed_thread = std::sync::Arc::clone(&killed);
    let key_thread = thread::spawn(move || -> Result<()> {
        while !killed_thread.load(Ordering::Acquire) {
            if event::poll(Duration::from_millis(100))? {
                if let CEvent::Key(key) = event::read()? {
                    key_tx.send(Event::Input(key))?;
                }
            }
        }
        Ok(())
    });
    threads.push(key_thread);

    loop {
        match rx.recv()? {
            Event::Update(host_id, update) => {
                match update {
                    Update::Result(duration) => app.update(host_id, Some(duration)),
                    Update::Timeout => app.update(host_id, None),
                    Update::Unknown => (),
                };
                terminal.draw(|f| {
                    // Split our
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints(
                            iter::repeat(Constraint::Length(1))
                                .take(app.data.len())
                                .chain(iter::once(Constraint::Percentage(10)))
                                .collect::<Vec<_>>()
                                .as_ref(),
                        )
                        .split(f.size());

                    let total_chunks = chunks.len();

                    let header_chunks = chunks[0..total_chunks - 1].to_owned();
                    let chart_chunk = chunks[total_chunks - 1].to_owned();

                    for (plot_data, chunk) in app.data.iter().zip(header_chunks) {
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
                            .split(chunk);

                        for (area, paragraph) in
                            header_layout.into_iter().zip(plot_data.header_stats())
                        {
                            f.render_widget(paragraph, area);
                        }
                    }

                    let datasets: Vec<Dataset> = app.data.iter().map(|d| d.into()).collect();

                    let y_axis_bounds = app.y_axis_bounds();

                    let chart = Chart::new(datasets)
                        .block(Block::default().borders(Borders::NONE))
                        .x_axis(Axis::default().style(Style::default().fg(Color::Gray)).bounds(app.x_axis_bounds()))
                        .y_axis(
                            Axis::default()
                                .style(Style::default().fg(Color::Gray))
                                .bounds(y_axis_bounds)
                                .labels(app.y_axis_labels(y_axis_bounds)),
                        );

                    f.render_widget(chart, chart_chunk)
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

    for thread in threads {
        thread.join().unwrap()?;
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

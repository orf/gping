use crate::plot_data::PlotData;
use anyhow::{anyhow, Result};
use chrono::prelude::*;
use clap::Parser;
use crossterm::event::KeyModifiers;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, SetSize},
};
use dns_lookup::lookup_host;
use itertools::{Itertools, MinMaxResult};
use pinger::{ping_with_interval, PingResult};
use std::io;
use std::io::BufWriter;
use std::iter;
use std::net::IpAddr;
use std::ops::Add;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::Span;
use tui::widgets::{Axis, Block, Borders, Chart, Dataset};
use tui::Terminal;

mod colors;
mod plot_data;
mod region_map;

use colors::Colors;
use shadow_rs::{formatcp, shadow};

shadow!(build);

const VERSION_INFO: &str = formatcp!(
    r#"{}
commit_hash: {}
build_time: {}
build_env: {},{}"#,
    build::PKG_VERSION,
    build::SHORT_COMMIT,
    build::BUILD_TIME,
    build::RUST_VERSION,
    build::RUST_CHANNEL
);

#[derive(Parser, Debug)]
#[command(author, name = "gping", about = "Ping, but with a graph.", version = VERSION_INFO)]
struct Args {
    #[arg(
        long,
        help = "Graph the execution time for a list of commands rather than pinging hosts"
    )]
    cmd: bool,
    #[arg(
        short = 'n',
        long,
        help = "Watch interval seconds (provide partial seconds like '0.5'). Default for ping is 0.2, default for cmd is 0.5."
    )]
    watch_interval: Option<f32>,
    #[arg(
        help = "Hosts or IPs to ping, or commands to run if --cmd is provided. Can use cloud shorthands like aws:eu-west-1."
    )]
    hosts_or_commands: Vec<String>,
    #[arg(
        short,
        long,
        default_value = "30",
        help = "Determines the number of seconds to display in the graph."
    )]
    buffer: u64,
    /// Resolve ping targets to IPv4 address
    #[arg(short = '4', conflicts_with = "ipv6")]
    ipv4: bool,
    /// Resolve ping targets to IPv6 address
    #[arg(short = '6', conflicts_with = "ipv4")]
    ipv6: bool,
    /// Interface to use when pinging.
    #[arg(short = 'i', long)]
    interface: Option<String>,
    #[arg(short = 's', long, help = "Uses dot characters instead of braille")]
    simple_graphics: bool,
    #[arg(
        long,
        help = "Vertical margin around the graph (top and bottom)",
        default_value = "1"
    )]
    vertical_margin: u16,
    #[arg(
        long,
        help = "Horizontal margin around the graph (left and right)",
        default_value = "0"
    )]
    horizontal_margin: u16,
    #[arg(
        name = "color",
        short = 'c',
        long = "color",
        use_value_delimiter = true,
        value_delimiter = ',',
        help = r#"Assign color to a graph entry. 

This option can be defined more than once as a comma separated string, and the 
order which the colors are provided will be matched against the hosts or 
commands passed to gping.

Hexadecimal RGB color codes are accepted in the form of '#RRGGBB' or the 
following color names: 'black', 'red', 'green', 'yellow', 'blue', 'magenta',
'cyan', 'gray', 'dark-gray', 'light-red', 'light-green', 'light-yellow', 
'light-blue', 'light-magenta', 'light-cyan', and 'white'"#
    )]
    color_codes_or_names: Vec<String>,
    #[arg(
        name = "clear",
        long = "clear",
        help = "\
            Clear the graph from the terminal after closing the program\
        ",
        action
    )]
    clear: bool,
}

struct App {
    data: Vec<PlotData>,
    display_interval: chrono::Duration,
    started: chrono::DateTime<Local>,
}

impl App {
    fn new(data: Vec<PlotData>, buffer: u64) -> Self {
        App {
            data,
            display_interval: chrono::Duration::from_std(Duration::from_secs(buffer)).unwrap(),
            started: Local::now(),
        }
    }

    fn update(&mut self, host_idx: usize, item: Option<Duration>) {
        let host = &mut self.data[host_idx];
        host.update(item);
    }

    fn y_axis_bounds(&self) -> [f64; 2] {
        // Find the Y axis bounds for our chart.
        // This is trickier than the x-axis. We iterate through all our PlotData structs
        // and find the min/max of all the values. Then we add a 10% buffer to them.
        let (min, max) = match self
            .data
            .iter()
            .flat_map(|b| b.data.as_slice())
            .map(|v| v.1)
            .filter(|v| !v.is_nan())
            .minmax()
        {
            MinMaxResult::NoElements => (f64::INFINITY, 0_f64),
            MinMaxResult::OneElement(elm) => (elm, elm),
            MinMaxResult::MinMax(min, max) => (min, max),
        };

        // Add a 10% buffer to the top and bottom
        let max_10_percent = (max * 10_f64) / 100_f64;
        let min_10_percent = (min * 10_f64) / 100_f64;
        [min - min_10_percent, max + max_10_percent]
    }

    fn x_axis_bounds(&self) -> [f64; 2] {
        let now = Local::now();
        let now_idx;
        let before_idx;
        if (now - self.started) < self.display_interval {
            now_idx = (self.started + self.display_interval).timestamp_millis() as f64 / 1_000f64;
            before_idx = self.started.timestamp_millis() as f64 / 1_000f64;
        } else {
            now_idx = now.timestamp_millis() as f64 / 1_000f64;
            let before = now - self.display_interval;
            before_idx = before.timestamp_millis() as f64 / 1_000f64;
        }

        [before_idx, now_idx]
    }

    fn x_axis_labels(&self, bounds: [f64; 2]) -> Vec<Span> {
        let lower_utc = NaiveDateTime::from_timestamp_opt(bounds[0] as i64, 0)
            .expect("Error parsing x-axis bounds 0");
        let upper_utc = NaiveDateTime::from_timestamp_opt(bounds[1] as i64, 0)
            .expect("Error parsing x-asis bounds 1");
        let lower = Local::from_utc_datetime(&Local, &lower_utc);
        let upper = Local::from_utc_datetime(&Local, &upper_utc);
        let diff = (upper - lower) / 2;
        let midpoint = lower + diff;
        vec![
            Span::raw(format!("{:?}", lower.time())),
            Span::raw(format!("{:?}", midpoint.time())),
            Span::raw(format!("{:?}", upper.time())),
        ]
    }

    fn y_axis_labels(&self, bounds: [f64; 2]) -> Vec<Span> {
        // Create 7 labels for our y axis, based on the y-axis bounds we computed above.
        let min = bounds[0];
        let max = bounds[1];

        let difference = max - min;
        let num_labels = 7;
        // Split difference into one chunk for each of the 7 labels
        let increment = Duration::from_micros((difference / num_labels as f64) as u64);
        let duration = Duration::from_micros(min as u64);

        (0..num_labels)
            .map(|i| Span::raw(format!("{:?}", duration.add(increment * i))))
            .collect()
    }
}

#[derive(Debug)]
enum Update {
    Result(Duration),
    Timeout,
    Unknown,
    Terminated(ExitStatus, String),
}

impl From<PingResult> for Update {
    fn from(result: PingResult) -> Self {
        match result {
            PingResult::Pong(duration, _) => Update::Result(duration),
            PingResult::Timeout(_) => Update::Timeout,
            PingResult::Unknown(_) => Update::Unknown,
            PingResult::PingExited(e, stderr) => Update::Terminated(e, stderr),
        }
    }
}

#[derive(Debug)]
enum Event {
    Update(usize, Update),
    Terminate,
    Render,
}

fn start_render_thread(
    kill_event: Arc<AtomicBool>,
    cmd_tx: Sender<Event>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        while !kill_event.load(Ordering::Acquire) {
            sleep(Duration::from_millis(250));
            cmd_tx.send(Event::Render)?;
        }
        Ok(())
    })
}

fn start_cmd_thread(
    watch_cmd: &str,
    host_id: usize,
    watch_interval: Option<f32>,
    cmd_tx: Sender<Event>,
    kill_event: Arc<AtomicBool>,
) -> JoinHandle<Result<()>> {
    let mut words = watch_cmd.split_ascii_whitespace();
    let cmd = words
        .next()
        .expect("Must specify a command to watch")
        .to_string();
    let cmd_args = words.map(|w| w.to_string()).collect::<Vec<String>>();

    let interval = Duration::from_millis((watch_interval.unwrap_or(0.5) * 1000.0) as u64);

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
            sleep(interval);
        }
        Ok(())
    })
}

fn start_ping_thread(
    host: String,
    host_id: usize,
    watch_interval: Option<f32>,
    ping_tx: Sender<Event>,
    kill_event: Arc<AtomicBool>,
    interface: Option<String>,
) -> Result<JoinHandle<Result<()>>> {
    let interval = Duration::from_millis((watch_interval.unwrap_or(0.2) * 1000.0) as u64);
    // Pump ping messages into the queue
    let stream = ping_with_interval(host, interval, interface)?;
    Ok(thread::spawn(move || -> Result<()> {
        while !kill_event.load(Ordering::Acquire) {
            match stream.recv() {
                Ok(v) => {
                    ping_tx.send(Event::Update(host_id, v.into()))?;
                }
                Err(_) => {
                    // Stream closed, just break
                    return Ok(());
                }
            }
        }
        Ok(())
    }))
}

fn get_host_ipaddr(host: &str, force_ipv4: bool, force_ipv6: bool) -> Result<String> {
    let ipaddr: Vec<IpAddr> = match lookup_host(host) {
        Ok(ip) => ip,
        Err(e) => return Err(anyhow!("Could not resolve hostname {}", host).context(e)),
    };
    let ipaddr = if force_ipv4 {
        ipaddr
            .iter()
            .find(|ip| matches!(ip, IpAddr::V4(_)))
            .ok_or_else(|| anyhow!("Could not resolve '{}' to IPv4", host))
    } else if force_ipv6 {
        ipaddr
            .iter()
            .find(|ip| matches!(ip, IpAddr::V6(_)))
            .ok_or_else(|| anyhow!("Could not resolve '{}' to IPv6", host))
    } else {
        ipaddr
            .first()
            .ok_or_else(|| anyhow!("Could not resolve '{}' to IP", host))
    };
    Ok(ipaddr?.to_string())
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    if args.hosts_or_commands.is_empty() {
        return Err(anyhow!("At least one host or command must be given (i.e gping google.com). Use --help for a full list of arguments."));
    }

    let mut data = vec![];

    let colors = Colors::from(args.color_codes_or_names.iter());
    let hosts_or_commands: Vec<String> = args
        .hosts_or_commands
        .clone()
        .into_iter()
        .map(|s| match region_map::try_host_from_cloud_region(&s) {
            None => s,
            Some(new_domain) => new_domain,
        })
        .collect();

    for (host_or_cmd, color) in hosts_or_commands.iter().zip(colors) {
        let color = color?;
        let display = match args.cmd {
            true => host_or_cmd.to_string(),
            false => format!(
                "{} ({})",
                host_or_cmd,
                get_host_ipaddr(host_or_cmd, args.ipv4, args.ipv6)?
            ),
        };
        data.push(PlotData::new(
            display,
            args.buffer,
            Style::default().fg(color),
            args.simple_graphics,
        ));
    }

    let (key_tx, rx) = mpsc::channel();

    let mut threads = vec![];

    let killed = Arc::new(AtomicBool::new(false));

    for (host_id, host_or_cmd) in hosts_or_commands.iter().cloned().enumerate() {
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
                args.watch_interval,
                key_tx.clone(),
                std::sync::Arc::clone(&killed),
                args.interface.clone(),
            )?);
        }
    }
    threads.push(start_render_thread(
        std::sync::Arc::clone(&killed),
        key_tx.clone(),
    ));

    let mut app = App::new(data, args.buffer);
    enable_raw_mode()?;
    let stdout = io::stdout();
    let mut backend = CrosstermBackend::new(BufWriter::with_capacity(1024 * 1024 * 4, stdout));
    let rect = backend.size()?;

    if args.clear {
        execute!(
            backend,
            SetSize(rect.width, rect.height),
            EnterAlternateScreen,
        )?;
    } else {
        execute!(backend, SetSize(rect.width, rect.height),)?;
    }

    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Pump keyboard messages into the queue
    let killed_thread = std::sync::Arc::clone(&killed);
    thread::spawn(move || -> Result<()> {
        while !killed_thread.load(Ordering::Acquire) {
            if event::poll(Duration::from_secs(5))? {
                if let CEvent::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            key_tx.send(Event::Terminate)?;
                            break;
                        }
                        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                            key_tx.send(Event::Terminate)?;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    });

    loop {
        match rx.recv()? {
            Event::Update(host_id, update) => {
                match update {
                    Update::Result(duration) => app.update(host_id, Some(duration)),
                    Update::Timeout => app.update(host_id, None),
                    Update::Unknown => (),
                    Update::Terminated(e, _) if e.success() => {
                        break;
                    }
                    Update::Terminated(e, stderr) => {
                        eprintln!("There was an error running ping: {e}\nStderr: {stderr}\n");
                        break;
                    }
                };
            }
            Event::Render => {
                terminal.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .vertical_margin(args.vertical_margin)
                        .horizontal_margin(args.horizontal_margin)
                        .constraints(
                            iter::repeat(Constraint::Length(1))
                                .take(app.data.len())
                                .chain(iter::once(Constraint::Percentage(10)))
                                .collect::<Vec<_>>()
                                .as_ref(),
                        )
                        .split(f.size());

                    let total_chunks = chunks.len();

                    let header_chunks = &chunks[0..total_chunks - 1];
                    let chart_chunk = &chunks[total_chunks - 1];

                    for (plot_data, chunk) in app.data.iter().zip(header_chunks) {
                        let header_layout = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints(
                                [
                                    Constraint::Percentage(30),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                    Constraint::Percentage(10),
                                ]
                                .as_ref(),
                            )
                            .split(*chunk);

                        for (area, paragraph) in header_layout.iter().zip(plot_data.header_stats())
                        {
                            f.render_widget(paragraph, *area);
                        }
                    }

                    let datasets: Vec<Dataset> = app.data.iter().map(|d| d.into()).collect();

                    let y_axis_bounds = app.y_axis_bounds();
                    let x_axis_bounds = app.x_axis_bounds();

                    let chart = Chart::new(datasets)
                        .block(Block::default().borders(Borders::NONE))
                        .x_axis(
                            Axis::default()
                                .style(Style::default().fg(Color::Gray))
                                .bounds(x_axis_bounds)
                                .labels(app.x_axis_labels(x_axis_bounds)),
                        )
                        .y_axis(
                            Axis::default()
                                .style(Style::default().fg(Color::Gray))
                                .bounds(y_axis_bounds)
                                .labels(app.y_axis_labels(y_axis_bounds)),
                        );

                    f.render_widget(chart, *chart_chunk)
                })?;
            }
            Event::Terminate => {
                killed.store(true, Ordering::Release);
                break;
            }
        }
    }
    killed.store(true, Ordering::Relaxed);

    disable_raw_mode()?;
    execute!(terminal.backend_mut())?;
    terminal.show_cursor()?;

    let new_size = terminal.size()?;
    terminal.set_cursor(new_size.width, new_size.height)?;
    for thread in threads {
        thread.join().unwrap()?;
    }

    if args.clear {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    };

    Ok(())
}

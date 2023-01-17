use clap::Parser;
// use shadow_rs::{formatcp, shadow};
//
// shadow!(build);
//
// pub const VERSION_INFO: &str = formatcp!(
//     r#"{}
// commit_hash: {}
// build_time: {}
// build_env: {},{}"#,
//     build::PKG_VERSION,
//     build::SHORT_COMMIT,
//     build::BUILD_TIME,
//     build::RUST_VERSION,
//     build::RUST_CHANNEL
// );

#[derive(Parser, Debug)]
#[command(author, name = "gping", about = "Ping, but with a graph.")]
pub struct Args {
    #[arg(
        long,
        help = "Graph the execution time for a list of commands rather than pinging hosts"
    )]
    pub cmd: bool,
    #[arg(
        short = 'n',
        long,
        help = "Watch interval seconds (provide partial seconds like '0.5'). Default for ping is 0.2, default for cmd is 0.5."
    )]
    pub watch_interval: Option<f32>,
    #[arg(
        help = "Hosts or IPs to ping, or commands to run if --cmd is provided. Can use cloud shorthands like aws:eu-west-1."
    )]
    pub hosts_or_commands: Vec<String>,
    #[arg(
        short,
        long,
        default_value = "30",
        help = "Determines the number of seconds to display in the graph."
    )]
    pub buffer: u64,
    /// Resolve ping targets to IPv4 address
    #[arg(short = '4', conflicts_with = "ipv6")]
    pub ipv4: bool,
    /// Resolve ping targets to IPv6 address
    #[arg(short = '6', conflicts_with = "ipv4")]
    pub ipv6: bool,
    /// Interface to use when pinging.
    #[arg(short = 'i', long)]
    pub interface: Option<String>,
    #[arg(short = 's', long, help = "Uses dot characters instead of braille")]
    pub simple_graphics: bool,
    #[arg(
        long,
        help = "Vertical margin around the graph (top and bottom)",
        default_value = "1"
    )]
    pub vertical_margin: u16,
    #[arg(
        long,
        help = "Horizontal margin around the graph (left and right)",
        default_value = "0"
    )]
    pub horizontal_margin: u16,
    #[arg(
        name = "color",
        short = 'c',
        long = "color",
        use_value_delimiter = true,
        value_delimiter = ',',
        help = "\
            Assign color to a graph entry. This option can be defined more than \
            once as a comma separated string, and the order which the colors are \
            provided will be matched against the hosts or commands passed to gping. \
            Hexadecimal RGB color codes are accepted in the form of '#RRGGBB' or the \
            following color names: 'black', 'red', 'green', 'yellow', 'blue', 'magenta',\
            'cyan', 'gray', 'dark-gray', 'light-red', 'light-green', 'light-yellow', \
            'light-blue', 'light-magenta', 'light-cyan', and 'white'\
        "
    )]
    pub color_codes_or_names: Vec<String>,
}
use clap::{Command, Args, FromArgMatches as _, Parser, Subcommand as _};

#[path = "src/args.rs"]
mod args;


fn main() -> shadow_rs::SdResult<()> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    let cli = clap::Command::new("Built CLI");
    // Augment with derived subcommands
    let cli = crate::args::Args::augment_args(cli);

    let man = clap_mangen::Man::new(cli);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(out_dir.join("mybin.1"), buffer)?;

    shadow_rs::new()
}

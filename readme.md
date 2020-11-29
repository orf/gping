# gping 🚀

[![Crates.io](https://img.shields.io/crates/v/gping.svg)](https://crates.io/crates/gping)
[![Actions Status](https://github.com/orf/gping/workflows/CI/badge.svg)](https://github.com/orf/gping/actions)

Ping, but with a graph.

![](./images/readme-example.gif)

Table of Contents
=================

   * [Install :cd:](#install-cd)
      * [Homebrew (MacOS   Linux)](#homebrew-macos)
      * [Binaries (Windows and ARM)](#binaries-windows-and-arm)
      * [Cargo](#cargo)
      * [Ubuntu/Debian](#apt-ubuntudebian)
   * [Usage :saxophone:](#usage-saxophone)

# Install :cd:

**This requires `rustc` version 1.44.0 or greater**

* Homebrew: `brew install gping`
* Linux (Homebrew): `brew install orf/brew/gping`
* Windows/ARM: Download the latest release from [the github releases page](https://github.com/orf/gping/releases)
* Fedora ([COPR](https://copr.fedorainfracloud.org/coprs/atim/gping/)): `sudo dnf copr enable atim/gping -y && sudo dnf install gping`
* Cargo: `cargo install gping`
* Ubuntu/Debian ([Azlux's repo](http://packages.azlux.fr/)): 
```bash
echo "deb http://packages.azlux.fr/debian/ buster main" | sudo tee /etc/apt/sources.list.d/azlux.list
wget -qO - https://azlux.fr/repo.gpg.key | sudo apt-key add -
apt update
apt install gping
```

# Usage :saxophone:

Just run `gping [host]`.

```bash
$ gping --help
gping 0.1.8
Ping, but with a graph.

USAGE:
    gping [FLAGS] [OPTIONS] [hosts-or-commands]...

FLAGS:
        --cmd        Graph the execution time for a list of commands rather than pinging hosts
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -b, --buffer <buffer>                    Determines the number pings to display. [default: 100]
    -n, --watch-interval <watch-interval>    Watch interval seconds (provide partial seconds like '0.5') [default: 0.5]

ARGS:
    <hosts-or-commands>...    Hosts or IPs to ping, or commands to run if --cmd is provided.
```

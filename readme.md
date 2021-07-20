# gping 🚀

[![Crates.io](https://img.shields.io/crates/v/gping.svg)](https://crates.io/crates/gping)
[![Actions Status](https://github.com/orf/gping/workflows/CI/badge.svg)](https://github.com/orf/gping/actions)

Ping, but with a graph.

![](./images/readme-example.gif)

Table of Contents
=================

   * [Install :cd:](#install-cd)
   * [Usage :saxophone:](#usage-saxophone)

# Install :cd:

* Homebrew: `brew install gping`
* Linux (Homebrew): `brew install orf/brew/gping`
* CentOS (and other distributions with an old glibc): Download the MUSL build from the latest release
* Windows/ARM: 
  * Scoop: `scoop install gping`
  * Chocolatey: `choco install gping`
  * Download the latest release from [the github releases page](https://github.com/orf/gping/releases)
* Fedora ([COPR](https://copr.fedorainfracloud.org/coprs/atim/gping/)): `sudo dnf copr enable atim/gping -y && sudo dnf install gping`
* Cargo (**This requires `rustc` version 1.44.0 or greater**): `cargo install gping`
* Arch Linux: `pacman -S gping`
* Ubuntu/Debian ([Azlux's repo](http://packages.azlux.fr/)):
```bash
echo "deb http://packages.azlux.fr/debian/ buster main" | sudo tee /etc/apt/sources.list.d/azlux.list
wget -qO - https://azlux.fr/repo.gpg.key | sudo apt-key add -
sudo apt update
sudo apt install gping
```
* Gentoo ([dm9pZCAq overlay](https://github.com/gentoo-mirror/dm9pZCAq)):
```sh
sudo eselect repository enable dm9pZCAq
sudo emerge --sync dm9pZCAq
sudo emerge net-misc/gping::dm9pZCAq
```

# Usage :saxophone:

Just run `gping [host]`.

```bash
$ gping --help
gping 1.2.0
Ping, but with a graph.

USAGE:
    gping [FLAGS] [OPTIONS] [hosts-or-commands]...

FLAGS:
        --cmd                Graph the execution time for a list of commands rather than pinging hosts
    -h, --help               Prints help information
    -4                       Resolve ping targets to IPv4 address
    -6                       Resolve ping targets to IPv6 address
    -s, --simple-graphics    Uses dot characters instead of braille
    -V, --version            Prints version information

OPTIONS:
    -b, --buffer <buffer>                    Determines the number of seconds to display in the graph. [default: 30]
    -n, --watch-interval <watch-interval>    Watch interval seconds (provide partial seconds like '0.5') [default: 0.5]

ARGS:
    <hosts-or-commands>...    Hosts or IPs to ping, or commands to run if --cmd is provided.
```

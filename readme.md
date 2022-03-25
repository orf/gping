# gping 🚀

[![Crates.io](https://img.shields.io/crates/v/gping.svg)](https://crates.io/crates/gping)
[![Actions Status](https://github.com/orf/gping/workflows/CI/badge.svg)](https://github.com/orf/gping/actions)

Ping, but with a graph.

![](./images/readme-example.gif)

Comes with the following super-powers:
* Graph the ping time for multiple hosts
* Graph the _execution time_ for commands via the `--cmd` flag
* Custom colours
* Windows, Mac and Linux support

Table of Contents
=================

   * [Install :cd:](#install-cd)
   * [Usage :saxophone:](#usage-saxophone)

<a href="https://repology.org/project/gping/versions">
    <img src="https://repology.org/badge/vertical-allrepos/gping.svg" alt="Packaging status" align="right">
</a>

# Install :cd:

* macOS
  * [Homebrew](https://formulae.brew.sh/formula/gping#default): `brew install gping`
  * [MacPorts](https://ports.macports.org/port/gping/): `sudo port install gping`
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
gping 1.3.0
Ping, but with a graph.

USAGE:
    gping [FLAGS] [OPTIONS] <hosts-or-commands>...

FLAGS:
        --cmd                Graph the execution time for a list of commands rather than pinging hosts
    -h, --help               Prints help information
    -4                       Resolve ping targets to IPv4 address
    -6                       Resolve ping targets to IPv6 address
    -s, --simple-graphics    Uses dot characters instead of braille
    -V, --version            Prints version information

OPTIONS:
    -b, --buffer <buffer>
            Determines the number of seconds to display in the graph. [default: 30]

    -c, --color <color>...
            Assign color to a graph entry. This option can be defined more than once and the order which the colors are
            provided will be matched against the hosts or commands passed to gping. Hexadecimal RGB color codes are
            accepted in the form of '#RRGGBB' or the following color names: 'black', 'red', 'green', 'yellow', 'blue',
            'magenta', 'cyan', 'gray', 'dark-gray', 'light-red', 'light-green', 'light-yellow', 'light-blue', 'light-
            magenta', 'light-cyan', and 'white'
        --horizontal-margin <horizontal-margin>    Horizontal margin around the graph (left and right) [default: 0]
        --vertical-margin <vertical-margin>        Vertical margin around the graph (top and bottom) [default: 1]
    -n, --watch-interval <watch-interval>
            Watch interval seconds (provide partial seconds like '0.5'). Default for ping is 0.2, default for cmd is
            0.5.

ARGS:
    <hosts-or-commands>...    Hosts or IPs to ping, or commands to run if --cmd is provided.
```

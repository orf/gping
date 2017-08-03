# gping
Ping, but with a graph

![](/doc/readme_screencast.gif)

[![Snap Status](https://build.snapcraft.io/badge/orf/gping.svg)](https://build.snapcraft.io/user/orf/gping)

## Install and run
Created/tested with Python 3.4 and 2.7.

`pip3 install pinggraph`

Tested on Windows and Ubuntu, should run on OS X as well. After installation just run:

`gping [yourhost]`

If you don't give a host then it pings google.

## Why?
My apartments internet is all 4g, and while it's normally pretty fast it can be a bit flakey. I often
found myself running `ping -t google.com` in a command window to get a rough idea of the network speed,
and I thought a graph would be a great way to visualize the data. I still wanted to just use the command
line though, so I decided to try and write a cross platform one that I could use. And here we are.


## Code
For a quick hack the code started off really nice, but after I decided pretty colors
were a good addition it quickly got rather complicated. Inside pinger.py
is a function `plot()`, this uses a canvas-like object to "draw" things like lines
and boxes to the screen. I found on Windows that changing the colors is slow and
caused the screen to flicker, so theres a big mess of a function called `process_colors`
to try and optimize that. Don't ask.

# gping
Ping, but with a graph

![](/doc/readme_screencast.gif)

## Install and run
Created/tested with Python 3.4, requires the `staticstics` module on 2.7.

`pip3 install pinggraph`

Tested on Windows and Ubuntu, should run on MacOS as well. After installation just run:

`gping [yourhost]`

If you don't give a host then it pings google.

## Code
For a quick hack the code started off really nice, but after I decided pretty colors
were a good addition it quickly got rather complicated. Inside pinger.py
is a function `plot()`, this uses a canvas-like object to "draw" things like lines
and boxes to the screen. I found on Windows that changing the colors is slow and
caused the screen to flicker, so theres a big mess of a function called `process_colors`
to try and optimize that. Don't ask.
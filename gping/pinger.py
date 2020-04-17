#!/usr/bin/env python
# coding=utf8

import functools
import itertools
import platform
import re
import subprocess
import sys
from collections import namedtuple, deque
from itertools import islice
import signal
import math

from colorama import Fore, init

__version__ = "0.0.13"

init()

buff = deque(maxlen=400)

Point = namedtuple("Point", "x y")

try:
    from gping.termsize import get_terminal_size
except ImportError:
    # python 2 compatibility
    from termsize import get_terminal_size

try:
    from colorama.ansitowin32 import winterm
except Exception:
    winterm = None


class Canvas(object):
    def __init__(self, width, height):
        # Each item in each row is a tuple, the first element is the single character that will be printed
        # and the second is the characters color.
        self.data = [ [(" ", None) for i in range(width)] for i in range(height - 1) ]

    def __setitem__(self, key, value):
        x, y = key

        if isinstance(value, tuple):
            data, color = value
            if callable(color):
                color = color(x, y)

            data = (data, color)
        else:
            data = (value, None)

        self.data[int(-y)][int(x)] = data

    def horizontal_line(self, data, row, from_, to, paint=None):
        if len(data) == 1:
            data = itertools.cycle(data)

        from_ = int(from_)
        to = int(to)
        data_iter = iter(data)
        for x in range(from_, to):
            self[x, row] = (next(data_iter, " "), paint)

    def vertical_line(self, data, column, from_, to, paint=None):
        if len(data) == 1:
            data = itertools.cycle(data)

        from_ = int(from_)
        to = int(to)
        data_iter = iter(data)
        for y in range(from_, to + 1):
            self[column, y] = (next(data_iter), paint)

    def line(self, from_, to, paint=None, character=None):
        from_, to = sorted([from_, to])

        if from_.x == to.x:
            character = character or "|"
            self.vertical_line(character, from_.x, from_.y, to.y, paint)
        elif from_.y == to.y:
            # Horizontal line. Just fill in the right buffer
            character = character or "-"
            self.horizontal_line(character, from_.y, from_.x, to.x, paint)

    def box(self, bottom_left_corner, top_right_corner, paint=None, blank=False):
        ''' creates the visual frame/box in which we place the graph '''
        path = [
            bottom_left_corner,
            Point(bottom_left_corner.x, top_right_corner.y),
            top_right_corner,
            Point(top_right_corner.x, bottom_left_corner.y),
            bottom_left_corner
        ]

        # use the bottom left corner as the starting point
        last_point = bottom_left_corner
        for idx, point in enumerate(path):
            # skipping the first item because we use it as starting point
            if idx != 0:
                self.line(last_point, point, paint=paint, character=" " if blank else None)
            last_point = point

    @property
    def lines(self):
        for line in self.data:
            data = []
            current_color = None
            # Output lines but keep track of color changes. Needed to stop colors bleeding over to other characters.
            # Only output color changes when needed, keeps this snappy on windows.
            for char, color in line:
                if color != current_color and char != " ":
                    if color is None:
                        data.append(Fore.RESET)
                    else:
                        data.append(color)
                    current_color = color
                data.append(char)

            yield "".join(data)


def plot(width, height, data, host):
    # We space for a newline after each line, so restrict the hight/width a bit
    width, height = width - 1, height - 1  # Our work area is slightly smaller
    canvas = Canvas(width, height)
    # Draw a box around the edges of the screen, one character in.
    canvas.box(
        Point(1, 1), Point(width - 1, height - 1)
    )
    # We use islice to slice the data because we can't do a ranged slice on a dequeue :(
    data_slice = list(islice(data, 0, width - 3))
    if not data_slice:
        return canvas

    max_ping = max(data)

    # Scale the chart.
    min_scaled, max_scaled = 0, height - 4

    try:
        yellow_zone_idx = round(max_scaled * (100 / max_ping))
        green_zone_idx = round(max_scaled * (50 / max_ping))
    except ZeroDivisionError:
        # It is 2 so the bottom block becomes green and not red
        yellow_zone_idx = 2
        green_zone_idx = 2

    for column, datum in enumerate(data_slice):
        if datum == -1:
            # If it's a timeout then do a red questionmark
            canvas[column + 2, 2] = ("?", Fore.RED)
            continue

        # Only draw a bar if the max_ping has been more than 0
        if max_ping > 0:
           # What percentage of the max_ping are we? 0 -> 1
           percent = min(datum / max_ping, 100) if datum < max_ping else 1
           bar_height = round(max_scaled * percent)
        else:
           percent = 0
           bar_height = 0

        # Our paint callback, we check if the y value of the point is in any of our zones,
        # if it is paint the appropriate color.
        def _paint(x, y):
            if y <= green_zone_idx:
                return Fore.GREEN
            elif y <= yellow_zone_idx:
                return Fore.YELLOW
            else:
                return Fore.RED  # Danger zone

        canvas.vertical_line(
            u"█", column + 2, 2, 2 + bar_height, paint=_paint
        )

    render_stats(data, canvas, width, height)

    # adding the url to the top
    if host:
        host = " {} ".format(host)
        from_url = width / 2 - round(len(host) / 2)
        to_url = from_url + len(host)
        canvas.horizontal_line(host, height - 1, from_url, to_url)

    return canvas

def render_stats(all_pings, canvas, width, height):
    valid_pings = sorted( [ pt for pt in all_pings if pt != -1 ] )
    valid_pings_last_n = sorted( [ pt for pt in list(all_pings)[:width - 2] if pt != -1 ] )

    if not valid_pings:
        return

    stats = [
      ("Last", "{} samples".format(len(valid_pings))),
      ("Cur", int(valid_pings[0])),
      ("p95", int(percentile(valid_pings, 95))),
      ("p50", int(percentile(valid_pings, 50))),
      ("p5", int(percentile(valid_pings, 5))),
      ("Loss", percent_lost(valid_pings)),
      None,
      ("Last", "{} samples".format(len(valid_pings_last_n))),
      ("max", int(percentile(valid_pings_last_n, 100))),
      ("p95", int(percentile(valid_pings_last_n, 95))),
      ("p50", int(percentile(valid_pings_last_n, 50))),
      ("p5", int(percentile(valid_pings_last_n, 5))),
      ("Loss", percent_lost(valid_pings_last_n)),
    ]

    max_label_width = max(len(stat[0]) for stat in stats if stat)
    max_value_width = max(len(str(stat[1])) for stat in stats if stat)
    box_width = max_label_width + 2 + max_value_width
    box_height = len(stats)

    # Paint each of the statistics lines
    for line_num, stat in enumerate(stats):
        if stat is None:
            continue
        stat_text = "{label: >{label_width}}: {value: <{value_width}}".format(
              label=stat[0],
              label_width=max_label_width,
              value=str(stat[1]),
              value_width=max_value_width,
        )
        
        canvas.horizontal_line(stat_text,
                               box_height - line_num,
                               width - box_width - 1, width - 1)

def percent_lost(sampling):
    num_lost = sum( [ sample == -1 for sample in sampling ] )
    percent_lost = int(100.0 * num_lost / len(sampling))
    return str(percent_lost) + "%"

def percentile(series, percentile):
    """ Re-implementation of numpy.percentile to avoid pulling in additional dependencies.
    Assumes the input is sorted. Averages the two nearest data points. """
    if not series:
        return None

    idx = (len(series) - 1) * percentile / 100.0
    return (series[int(math.floor(idx))] + series[int(math.ceil(idx))]) / 2

assert percentile([1], 50) == 1
assert percentile([1, 3, 5], 50) == 3
assert percentile([1, 3, 5], 75) == 4
assert percentile([1, 3, 5], 100) == 5

# A bunch of regexes nice people on github made.
windows_re = re.compile('.*?\\d+.*?\\d+.*?\\d+.*?\\d+.*?\\d+.*?(\\d+)', re.IGNORECASE | re.DOTALL)

linux_re = re.compile(r'time=(\d+(?:\.\d+)?) *ms', re.IGNORECASE)

darwin_re = re.compile(r'''
    \s?([0-9]*) # capture the bytes of data
    \sbytes\sfrom\s # bytes from
    (\d+\.\d+\.\d+\.\d+):
    \s+icmp_seq=(\d+)  # capture icmp_seq
    \s+ttl=(\d+)  # capture ttl
    \s+time=(?:([0-9\.]+)\s+ms)  # capture time
    ''',
                       re.VERBOSE | re.IGNORECASE | re.DOTALL)


# Simple ping decorator. Takes a list of default arguments to be passed to ping (used in the windows pinger)
def pinger(default_options=None, help="--help"):
    # We return the inner decorator (a bit verbose I know)
    def _wrapper(func):
        # Make the wrapped function...
        @functools.wraps(func)
        def _inner(args):
            if any(arg in ("--help", "/?") for arg in args):
                ping_help_text = subprocess.getoutput("ping {0}".format(help))
                print(Fore.GREEN + "gping {version}. Pass any parameters you would normally to ping".format(
                    version=__version__))
                print("Ping help:")
                print(Fore.RESET + ping_help_text)
                return

            args = ["ping"] + (default_options or []) + list(args)
            try:
                ping = subprocess.Popen(args, stdout=subprocess.PIPE)
            except PermissionError:
                print("Error running 'ping'. If you are using snap run snap connect gping:network-observe")
                return

            # Store the last 5 lines of output in case ping unexpectedly quits
            last_5 = deque(maxlen=5)
            while True:
                line = ping.stdout.readline().decode()

                if line == "":
                    print("ping quit unexpectedly. Last 5 lines of output:")
                    print("\n".join(last_5))
                    return

                last_5.append(line)
                result = func(line)
                # A none result means no result (i.e a header or something). -1 means timeout, otherwise it's the ping.
                if result is None:
                    continue

                yield result

        return _inner

    return _wrapper


@pinger(["-t"], help="/h")
def windows_ping(line):
    if line.startswith("Reply from"):
        return int(windows_re.search(line).group(1))
    elif "timed out" in line or "failure" in line:
        return -1


@pinger()
def linux_ping(line):
    if line.startswith("64 bytes from"):
        return round(float(linux_re.search(line).group(1)))
    else:
        return -1


@pinger()
def darwin_ping(line):
    if line.startswith("64 bytes from"):
        return round(float(darwin_re.search(line).group(5)))
    elif line.startswith("Request timeout"):
        return -1


def simulate_ping():
    import random
    last = random.randint(25, 150)
    while True:
        curr = random.randint(last - ((last / 10) * 20), last + ((last / 10) * 20))
        if not 25 < curr < 500:
            continue
        last = curr
        yield curr
        # time.sleep(0.1)


def run():
    # We do this so the command line stub installed by setup.py is surrounded by try/catch. Also neater than
    # wrapping the whole contents of _run().
    try:
        _run()
    except KeyboardInterrupt:
        pass

def draw(system, host):
    width, height = get_terminal_size()

    plotted = plot(width, height, buff, host)

    if winterm and system == "Windows":
        winterm.set_cursor_position((1, 1))
    if system == "Darwin":
        print("\033[%sA" % height)
    else:
        print(chr(27) + "[2J")

    print("\n".join(plotted.lines))

def clear():
    width, height = get_terminal_size()
    for i in range(height):
        print(" ")

def on_resize(system, host):
    clear()
    draw(system, host)

def _run():
    if len(sys.argv) == 1:
        options = ["google.com"]
        host = options[0]
    else:
        options = sys.argv[1:]
        host = sys.argv[-1]

    system = platform.system()
    if system == "Windows":
        it = windows_ping
    elif system == "Darwin":
        it = darwin_ping
    else:
        it = linux_ping

    if system != "Windows":
        signal.signal(signal.SIGWINCH, lambda _1, _2: on_resize(system, host))

    clear()
    for line in it(options):
        buff.appendleft(line)
        draw(system, host)


if __name__ == "__main__":
    run()

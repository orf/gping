import functools
import itertools
import platform
import re
import subprocess
import sys
from collections import namedtuple, deque
from itertools import islice

from colorama import Fore, init

__version__ = "0.0.12"

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
        self.data = [
            [(" ", None) for i in range(width)]
            for i in range(height)
            ]

    def __setitem__(self, key, value):
        x, y = key

        if isinstance(value, tuple):
            data, color = value
            if callable(color):
                color = color(x, y)

            data = (data, color)
        else:
            data = (value, None)

        self.data[-y][x] = data

    def horizontal_line(self, data, row, from_, to, paint=None):
        if len(data) == 1:
            data = itertools.cycle(data)

        from_ = int(from_)
        to = int(to)
        data_iter = iter(data)
        for x in range(from_, to):
            self[x, row] = (next(data_iter), paint)

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

    # Filter the -1's (timeouts) from the data so it doesn't impact avg, sum etc.
    filtered_data = [d for d in data if d != -1]
    if not filtered_data:
        return canvas

    average_ping = sum(filtered_data) / len(filtered_data)
    max_ping = max(filtered_data)

    if max_ping > (average_ping * 2):
        max_ping *= 0.75

    # Scale the chart.
    min_scaled, max_scaled = 0, height - 4

    yellow_zone_idx = round(max_scaled * (100 / max_ping))
    green_zone_idx = round(max_scaled * (50 / max_ping))

    for column, datum in enumerate(data_slice):
        if datum == -1:
            # If it's a timeout then do a red questionmark
            canvas[column + 2, 2] = ("?", Fore.RED)
            continue

        # What percentage of the max_ping are we? 0 -> 1
        percent = min(datum / max_ping, 100) if datum < max_ping else 1
        bar_height = round(max_scaled * percent)

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
            "█", column + 2, 2, 2 + bar_height, paint=_paint
        )

    stats_box = [
        "Avg: {:6.0f}".format(average_ping),
        "Min: {:6.0f}".format(min(filtered_data)),  # Filter None values
        "Max: {:6.0f}".format(max(filtered_data)),
        "Cur: {:6.0f}".format(filtered_data[0])
    ]
    # creating the box for the ping information in the middle
    midpoint = Point(
        round(width / 2),
        round(height / 2)
    )
    max_stats_len = max(len(s) for s in stats_box)
    # Draw a box around the outside of the stats box. We do this to stop the bars from touching the text,
    # it looks weird. We need a blank area around it.
    canvas.box(
        Point(midpoint.x - round(max_stats_len / 2) - 1, midpoint.y + len(stats_box)),
        Point(midpoint.x + round(max_stats_len / 2) - 1, midpoint.y - 1),
        blank=True
    )
    # Paint each of the statistics lines
    for idx, stat in enumerate(stats_box):
        from_stat = midpoint.x - round(max_stats_len / 2)
        to_stat = from_stat + len(stat)
        canvas.horizontal_line(stat, midpoint.y + idx, from_stat, to_stat)

    # adding the url to the top
    if host:
        host = " {} ".format(host)
        from_url = midpoint.x - round(len(host) / 2)
        to_url = from_url + len(host)
        canvas.horizontal_line(host, height - 1, from_url, to_url)

    return canvas


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
            ping = subprocess.Popen(args, stdout=subprocess.PIPE)
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


def _run():
    if len(sys.argv) == 1:
        options = ["google.com"]
        host = options[0]
    else:
        options = sys.argv[1:]
        host = ""

    system = platform.system()
    if system == "Windows":
        it = windows_ping
    elif system == "Darwin":
        it = darwin_ping
    else:
        it = linux_ping

    for line in it(options):
        buff.appendleft(line)

        width, height = get_terminal_size()

        plotted = plot(width, height, buff, host)

        if winterm and system == "Windows":
            winterm.set_cursor_position((1, 1))
        else:
            print(chr(27) + "[2J")

        print("\n".join(plotted.lines))


if __name__ == "__main__":
    run()

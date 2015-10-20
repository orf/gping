import os
import subprocess
import re
import collections
import itertools
import platform
import io

from colorama import Fore, init

try:
    from colorama.ansitowin32 import winterm
except Exception:
    winterm = None
import sys

try:
    from gping.termsize import get_terminal_size
except ImportError:
    #python 2 compatibility
    from termsize import get_terminal_size

init()
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

buff = collections.deque([0 for _ in range(20)], maxlen=400)

P = collections.namedtuple("Point", "x y")
hidden = object()


class Bitmap(object):
    def __init__(self, width, height, default=" "):
        ''' the pane on which we are oing to draw'''
        self.width = width
        self.height = height
        self._bitmap = [
            [default for _ in range(width + 1)]
            for _ in range(height + 1)
            ]
    def __getitem__(self, point):
        ''' we get the value at the given point
            raise an error if the passed item is not Point object or Integer object
        '''
        if isinstance(point, P):
            return self._bitmap[self.height - idx.y][idx.x]
        #
        elif not isinstance(point, int):
            raise RuntimeError("Can only index Bitmaps using an integer")

        return self._bitmap[self.height - idx]

    def __setitem__(self, point, value):
        ''' we set the value at the given point 
            raise an error if its not Point Object
        '''
        if isinstance(point, P):
            self._bitmap[self.height - int(point.y)][int(point.x)] = value
        else:
            raise RuntimeError("Can only use __setitem__ with a point")


class ConsoleCanvas(object):
    def __init__(self, width, height):
        self.bitmap = Bitmap(width, height)
        self.colors = Bitmap(width, height, default="")

    def point(self, point, data, paint=None):
        ''' Args :
                point - point where the data shall be placed
                data 
                paint
        '''
        self.bitmap[point] = data
        if isinstance(paint, str):
            self.colors[point] = paint
        else:
            self.colors[point] = paint(point) if paint else ""

    # Yes, these two methods could be refactored :/
    def horizontal_line(self, data, row, from_, to, paint=None):
        from_ = int(from_)
        to = int(to)
        data_iter = iter(data)
        for i in range(from_,to):
            p = P(i, row)
            self.point(p, next(data_iter), paint)

    def vertical_line(self, character, column, from_, to, paint=None):
        to = int(to)
        from_ = int(from_)
        for i in range(from_,to + 1):
            p = P(column, i)
            self.point(p, character, paint)

    def line(self, from_, to, paint=None, character=None):
        from_, to = sorted([from_, to])

        if from_.x == to.x:
            character = character or "|"
            self.vertical_line(character, from_.x, from_.y, to.y, paint)
        elif from_.y == to.y:
            # Horizontal line. Just fill in the right buffer
            character = character or "-"
            self.horizontal_line(itertools.cycle(character), from_.y, from_.x, to.x, paint)
        else:
            raise RuntimeError("Diagonal lines are not supported")

    def box(self, bottom_left_corner, top_right_corner, paint=None, blank=False):
        ''' creates the visual frame/box in which we place the graph '''
        path = [
            bottom_left_corner,
            P(bottom_left_corner.x, top_right_corner.y),
            top_right_corner,
            P(top_right_corner.x, bottom_left_corner.y),
            bottom_left_corner
        ]

        #use the bottom left corner as the starting point
        last_point = bottom_left_corner
        for idx, point in enumerate(path):
            #skipping the first item because we use it as starting point
            if idx != 0:
                self.line(last_point, point, paint=paint, character=" " if blank else None)
            last_point = point

    def process_colors(self):
        # Try and optimize colours. Maybe not needed on *nix?
        for row_idx, color_row in enumerate(self.colors._bitmap):
            last_color = None
            r = io.StringIO()
            for col_idx, color_item in enumerate(color_row):
                d = self.bitmap._bitmap[row_idx][col_idx]
                d = u'{}'.format(d)
                if d and d != " ":
                    if color_item:
                        if color_item != last_color:
                            r.write(u'{}'.format(color_item))
                        last_color = color_item
                    elif last_color:
                        r.write(u'{}'.format(Fore.RESET))
                    r.write(d if d is not hidden else " ")
                    if not color_item:
                        if last_color:
                            r.write(u'{}'.format(Fore.RESET))
                        last_color = None
                else:
                    r.write(d)
            yield r.getvalue()


def plot(url, data, width, height):
    canvas = ConsoleCanvas(width, height)
    canvas.box(
        P(1, 1), P(width, height)
    )

    data_slice = list(itertools.islice(data, 1, width - 3))
    stats_data = [d for d in data_slice if d]
    if not stats_data:
        return canvas

    max_ping = max(max(stats_data), 100)
    min_scaled, max_scaled = 0, height - 3

    yellow_zone_idx = round(max_scaled * (100 / max_ping))
    green_zone_idx = round(max_scaled * (50 / max_ping))

    for column, datum in enumerate(data_slice, 2):
        if datum is None:
            canvas.point(P(column, 2), "?", Fore.RED)
            continue
        elif datum is 0:
            continue
        # bar percentage
        percent = (datum / max_ping)
        # percent of max
        bar_height = round(max_scaled * percent)
        if bar_height == 0:
            bar_height = 1

        def _paint(point):
            y = point.y
            if y <= green_zone_idx:
                return Fore.GREEN
            elif y <= yellow_zone_idx:
                return Fore.YELLOW
            else:
                return Fore.RED

        canvas.vertical_line(
            "#", column, 2, 2 + bar_height, paint=_paint
        )

    if stats_data:
        average = sum(stats_data)/len(stats_data)
        stats_box = [
            "Avg: {:6.0f}".format(average),
            "Min: {:6.0f}".format(min(d for d in stats_data if d)),  # Filter None values
            "Max: {:6.0f}".format(max(stats_data)),
            "Cur: {:6.0f}".format(stats_data[0])
        ]
        max_stats_len = max(len(s) for s in stats_box)

        #when does this get to execution ?
        #from what it seems this doesnt get in
        if False:
            for idx, stat in enumerate(stats_box):
                canvas.horizontal_line(stat, height - 2 - idx, width - max_stats_len - 2)

            canvas.box(
                P(width - max_stats_len - len(stats_box), height - 2 - len(stats_box)),
                P(width - 1, height - 1)
            )
        else:
            #creating the box for the ping information in the middle
            midpoint = P(
                round(width / 2),
                round(height / 2)
            )

            canvas.box(
                P(midpoint.x - round(max_stats_len / 2) - 1, midpoint.y + len(stats_box)),
                P(midpoint.x + round(max_stats_len / 2) - 1, midpoint.y - 1),
                blank=True
            )

            for idx, stat in enumerate(stats_box):
                from_stat = midpoint.x - round(max_stats_len / 2)
                to_stat = from_stat + len(stat)
                canvas.horizontal_line(stat, midpoint.y + idx, from_stat ,to_stat )

            #adding the url to the top
            from_url = midpoint.x - round(len(url) / 2)
            to_url = from_url + len(url)
            canvas.horizontal_line(url, height, from_url ,to_url)

    return canvas


def _windows(url):
    ping = subprocess.Popen(["ping", "-t", url], stdout=subprocess.PIPE)
    while True:
        line = ping.stdout.readline().decode()
        if line.startswith("Reply from"):
            yield int(windows_re.search(line).group(1))
        elif "timed out" in line or "failure" in line:
            yield None


def _linux(url):
    ping = subprocess.Popen(["ping", url], stdout=subprocess.PIPE)
    while True:
        line = ping.stdout.readline().decode()
        if line.startswith("64 bytes from"):
            yield round(float(linux_re.search(line).group(1)))


def _darwin(url):
    ping = subprocess.Popen(["ping", url], stdout=subprocess.PIPE)
    while True:
        line = ping.stdout.readline().decode()
        if line.startswith("64 bytes from"):
            yield round(float(darwin_re.search(line).group(5)))
        elif line.startswith("Request timeout"):
            yield None


def _simulate(url):
    import time, random
    last = random.randint(25, 150)
    while True:
        curr = random.randint(last - ((last / 10) * 20), last + ((last / 10) * 20))
        if not 25 < curr < 150:
            continue
        last = curr
        yield curr
        time.sleep(0.1)


def _run():
    try:
        url = sys.argv[1]
    except IndexError:
        url = "google.com"

    if url == "--sim":
        it = _simulate
    else:
        system = platform.system()
        if system == "Windows":
            it = _windows
        elif system == "Darwin":
            it = _darwin
        else:
            it = _linux

    for ping in it(url):
        buff.appendleft(ping)
        if winterm:
            winterm.set_cursor_position((1, 1))
        else:
            os.system("cls" if platform.system() == "Windows" else "clear")
        width, height = get_terminal_size()
        c = plot(url, buff, width - 2, height - 2)
        print("\n".join(c.process_colors()))


def run():
    try:
        _run()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    run()

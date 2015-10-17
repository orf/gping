from setuptools import setup

setup(
    name='pinggraph',
    version='0.0.3',
    packages=['gping'],
    url='https://github.com/orf/gping',
    license='',
    author='Orf',
    author_email='tom@tomforb.es',
    description='Ping, but with a graph. Visit the Github page for more info',
    requires=['colorama'],
    entry_points={
        "console_scripts": [
            "gping=pinggraph.pinger:run"
        ]
    }
)

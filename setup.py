from setuptools import setup

setup(
    name='pinggraph',
    version='0.0.3',
    packages=['pinggraph'],
    url='',
    license='',
    author='Orf',
    author_email='tom@tomforb.es',
    description='', requires=['colorama'],
    entry_points={
        "console_scripts": [
            "gping=pinggraph.pinger:run"
        ]
    }
)

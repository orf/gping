use std::{iter::Iterator, ops::RangeFrom};

use anyhow::{anyhow, Result};
use read_color::rgb;
use tui::style::Color;

pub struct Colors<T> {
    already_used: Vec<Color>,
    color_names: T,
    indices: RangeFrom<u8>,
}

impl<T> From<T> for Colors<T> {
    fn from(color_names: T) -> Self {
        Self {
            already_used: Vec::new(),
            color_names,
            indices: 2..,
        }
    }
}

impl<'a, T> Iterator for Colors<T>
where
    T: Iterator<Item = &'a String>,
{
    type Item = Result<Color>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.color_names.next() {
            Some(name) => match try_color_from_string(name) {
                Ok(color) => {
                    if !self.already_used.contains(&color) {
                        self.already_used.push(color);
                    }
                    Some(Ok(color))
                }
                error => Some(error),
            },
            None => loop {
                let index = unsafe { self.indices.next().unwrap_unchecked() };
                let color = Color::Indexed(index);
                if !self.already_used.contains(&color) {
                    self.already_used.push(color);
                    break Some(Ok(color));
                }
            },
        }
    }
}

fn try_color_from_string(string: &str) -> Result<Color> {
    let mut characters = string.chars();

    let color = if let Some('#') = characters.next() {
        match rgb(&mut characters) {
            Some([r, g, b]) => Color::Rgb(r, g, b),
            None => return Err(anyhow!("Invalid color code: `{}`", string)),
        }
    } else {
        use Color::*;
        match string.to_lowercase().as_str() {
            "black" => Black,
            "red" => Red,
            "green" => Green,
            "yellow" => Yellow,
            "blue" => Blue,
            "magenta" => Magenta,
            "cyan" => Cyan,
            "gray" => Gray,
            "dark-gray" => DarkGray,
            "light-red" => LightRed,
            "light-green" => LightGreen,
            "light-yellow" => LightYellow,
            "light-blue" => LightBlue,
            "light-magenta" => LightMagenta,
            "light-cyan" => LightCyan,
            "white" => White,
            invalid => return Err(anyhow!("Invalid color name: `{}`", invalid)),
        }
    };

    Ok(color)
}

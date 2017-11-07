#[macro_use]
extern crate nom;
extern crate encoding;
extern crate termion;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use termion::{clear, cursor, style};
use termion::raw::IntoRawMode;
use termion::input::TermRead;
use termion::event::Key;

mod puzfile;

use puzfile::PuzFile;

#[derive(Debug)]
pub struct Cell {
    truth: Option<char>,
    guess: Option<char>,
    clue_number: Option<u16>,
    clue_across: Option<String>,
    clue_down: Option<String>,
}

pub struct Game<R, W: Write> {
    width: u16,
    height: u16,
    grid: Vec<Cell>,
    stdout: W,
    stdin: R,
}

fn init<R: Read, W: Write>(stdin: R, mut stdout: W, p: PuzFile) {
    let mut grid = Vec::new();

    for c in p.puzzle.chars() {
        let truth = match c {
            '.' => None,
            _ => Some(c),
        };

        grid.push(Cell {
            truth: truth,
            guess: None,
            clue_number: None,
            clue_across: None, // TODO
            clue_down: None, // TODO
        });
    }

    write!(stdout, "{}", clear::All).unwrap();

    let mut g = Game {
        width: p.width as u16,
        height: p.height as u16,
        grid: grid,
        stdout: stdout,
        stdin: stdin.keys(),
    };

    let mut clue_number = 1;
    let mut clue_index = 0;

    for y in 0..g.height {
        for x in 0..g.width {
            let across = g.has_clue_across(x, y);
            let down = g.has_clue_down(x, y);

            let c = g.get_mut(x, y);

            if across {
                c.clue_across = Some(p.clues[clue_index].clone());

                clue_index += 1;
            }

            if down {
                c.clue_down = Some(p.clues[clue_index].clone());

                clue_index += 1;
            }

            if across || down {
                c.clue_number = Some(clue_number);

                clue_number += 1;
            }
        }
    }

    g.draw_all();
}

impl<R, W: Write> Game<R, W> {
    fn get(&self, x: u16, y: u16) -> &Cell {
        return &self.grid[y as usize * self.width as usize + x as usize];
    }

    fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        return &mut self.grid[y as usize * self.width as usize + x as usize];
    }

    fn has_clue_across(&self, x: u16, y: u16) -> bool {
        if self.get(x, y).truth.is_none() {
            return false;
        }

        if x > 0 && self.get(x - 1, y).truth.is_some() {
            return false;
        }

        if x > self.width || self.get(x + 1, y).truth.is_none() {
            return false;
        }

        return true;
    }

    fn has_clue_down(&self, x: u16, y: u16) -> bool {
        if self.get(x, y).truth.is_none() {
            return false;
        }

        if y > 0 && self.get(x, y - 1).truth.is_some() {
            return false;
        }

        if y > self.height || self.get(x, y + 1).truth.is_none() {
            return false;
        }

        return true;
    }

    fn draw_cell(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 1)).unwrap();

        match self.get(x, y).truth {
            Some(t) => {
                match self.get(x, y).clue_number {
                    Some(n) => write!(self.stdout, "{:<3}\u{2503}", n).unwrap(),
                    None => write!(self.stdout, "   \u{2503}").unwrap(),
                };
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 2)).unwrap();
                write!(self.stdout, " {} \u{2503}", t).unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();
                write!(self.stdout, "\u{2501}\u{2501}\u{2501}\u{254B}").unwrap();
            }
            None => {
                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 2)).unwrap();
                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();
                write!(self.stdout, "\u{2501}\u{2501}\u{2501}\u{254B}").unwrap();
            }
        }
    }

    fn draw_all(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.draw_cell(x, y);
            }
        }

        write!(
            self.stdout,
            "{}",
            cursor::Goto(0, (self.height - 1) * 3 + 4)
        ).unwrap();
    }
}

fn main() {
    let mut f = File::open(&Path::new("daily-2006-05-11.puz")).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();

    let (_, p) = puzfile::parse_all(&v[..]).unwrap();

    let stdout = io::stdout();
    let stdout = stdout.lock();
    let stdout = stdout.into_raw_mode().unwrap();

    let stdin = io::stdin();
    let stdin = stdin.lock();

    init(stdin, stdout, p);
}

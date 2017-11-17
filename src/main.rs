#[macro_use]
extern crate nom;
extern crate encoding;
extern crate termion;
extern crate stopwatch;

use std::fs::File;
use std::env;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use termion::{async_stdin, clear, cursor, color, style};
use termion::raw::IntoRawMode;
use termion::input::TermRead;

use stopwatch::Stopwatch;

mod puzfile;

use puzfile::PuzFile;

#[derive(Copy, Clone)]
enum Mode {
    EditAcross,
    EditDown,
    Select,
    Pause,
}

#[derive(Debug)]
pub struct Cell {
    truth: Option<char>,
    guess: Option<char>,
    clue_number: Option<u16>,
    clue_across: Option<String>,
    clue_down: Option<String>,
}

pub struct Game<W: Write> {
    width: u16,
    height: u16,
    grid: Vec<Cell>,
    cursor_x: u16,
    cursor_y: u16,
    clues_scroll: u16,
    mode: Mode,
    last_edit_mode: Mode,
    stdout: W,
    stdin: termion::input::Keys<termion::AsyncReader>,
    stopwatch: Stopwatch,
    tick: u64,
}

pub struct GameStatus {
    cells: u16,
    guesses: u16,
    errors: u16,
}

fn init<W: Write>(stdin: termion::input::Keys<termion::AsyncReader>, mut stdout: W, p: &PuzFile) {
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
        width: u16::from(p.width),
        height: u16::from(p.height),
        grid: grid,
        cursor_x: 0,
        cursor_y: 0,
        clues_scroll: 0,
        mode: Mode::Select,
        last_edit_mode: Mode::EditAcross,
        stdout: stdout,
        stdin: stdin,
        stopwatch: Stopwatch::new(),
        tick: 0,
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
    g.start();
}

impl<W: Write> Drop for Game<W> {
    fn drop(&mut self) {
        // When done, restore the defaults to avoid messing with the terminal.
        write!(
            self.stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1)
        ).unwrap();
    }
}

impl<W: Write> Game<W> {
    fn get(&self, x: u16, y: u16) -> &Cell {
        &self.grid[y as usize * self.width as usize + x as usize]
    }

    fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        &mut self.grid[y as usize * self.width as usize + x as usize]
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

        true
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

        true
    }

    fn get_status(&self) -> GameStatus {
        let mut s = GameStatus {
            cells: 0,
            guesses: 0,
            errors: 0,
        };

        for cell in &self.grid {
            if cell.truth.is_some() {
                s.cells += 1;
            }

            if cell.guess.is_some() {
                s.guesses += 1;
            }

            match (cell.truth, cell.guess) {
                (Some(t), Some(g)) if t != g => s.errors += 1,
                _ => {}

            }
        }

        s
    }

    fn draw_cell(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 1)).unwrap();

        match self.get(x, y).truth {
            Some(_t) => {
                // Use an  arrow on the right border if this is the selected cell
                // and we're in Mode::EditAcross

                let right_border = match self.mode {
                    Mode::EditAcross if self.cursor_x == x && self.cursor_y == y => {
                        format!("{}\u{25B6}{}", color::Fg(color::LightRed), style::Reset)
                    }
                    _ => "\u{2503}".to_string(),
                };

                match self.get(x, y).clue_number {
                    Some(n) => write!(self.stdout, "{:<3}\u{2503}", n).unwrap(),
                    None => write!(self.stdout, "   \u{2503}").unwrap(),
                };
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 2)).unwrap();

                match self.get(x, y).guess {
                    Some(g) => {
                        write!(
                            self.stdout,
                            " {}{}{} {}",
                            style::Bold,
                            g,
                            style::Reset,
                            right_border
                        ).unwrap()
                    }
                    None => write!(self.stdout, "   {}", right_border).unwrap(),
                };
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();

                // Draw a downward-pointing arrow in the bottom border if this is the
                // selected cell and we're in Mode::EditDown

                match self.mode {
                    Mode::EditDown if self.cursor_x == x && self.cursor_y == y => {
                        write!(
                            self.stdout,
                            "\u{2501}{}\u{25BC}{}\u{2501}\u{254B}",
                            color::Fg(color::LightRed),
                            style::Reset
                        ).unwrap()
                    }
                    _ => write!(self.stdout, "\u{2501}\u{2501}\u{2501}\u{254B}").unwrap(),
                }
            }
            None => {
                // Draw a black cell

                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 2)).unwrap();
                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();
                write!(self.stdout, "\u{2501}\u{2501}\u{2501}\u{254B}").unwrap();
            }
        }
    }

    fn draw_cursor_cell(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.draw_cell(x, y);
    }

    fn draw_status_bar(&mut self) {
        let (term_width, term_height) = termion::terminal_size().unwrap();

        let s = self.get_status();

        write!(
            self.stdout,
            "{}{}{}{}{}",
            cursor::Goto(0, term_height),
            color::Bg(color::White),
            color::Fg(color::Black),
            " ".repeat(term_width as usize),
            cursor::Goto(0, term_height),
        ).unwrap();

        write!(
            self.stdout,
            "puzcli 0.1.0 G{}/{} E{} T{}:{:02}:{:02}",
            s.guesses,
            s.cells,
            s.errors,
            self.stopwatch.elapsed().as_secs() / 60 / 60,
            (self.stopwatch.elapsed().as_secs() / 60) % 60,
            self.stopwatch.elapsed().as_secs() % 60,
        ).unwrap();

        write!(self.stdout, "{}", style::Reset).unwrap();
    }

    fn draw_all(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.draw_cell(x, y);
            }
        }

        self.draw_clues();
        self.draw_status_bar();
        self.draw_cursor();

        self.stdout.flush().unwrap();
    }

    fn draw_clues(&mut self) {
        let (term_width, term_height) = termion::terminal_size().unwrap();

        let clues_width = term_width - self.width * 4 - 2;
        let clues_height = term_height - 1;

        // Across / Down labels aren't truncated, so they'll wrap into
        // the game board if we don't have enough space to display them.

        if clues_width < 6 {
            return;
        }

        let x = self.cursor_x;
        let y = self.cursor_y;
        let cursor_clue_number = self.get(x, y).clue_number;

        let mut strings = Vec::new();

        strings.push(format!("{}Across{}", style::Bold, style::Reset));
        strings.push("".into());

        for cell in &self.grid {
            if let Some(ref clue) = cell.clue_across {
                let mut tmp = format!("{}. {}", cell.clue_number.unwrap(), clue);
                tmp.truncate(clues_width as usize);

                match cursor_clue_number {
                    Some(n) if n == cell.clue_number.unwrap() => {
                        strings.push(format!("{}{}{}", style::Bold, tmp, style::Reset));
                    }
                    _ => strings.push(tmp),
                }
            }
        }

        strings.push("".into());
        strings.push(format!("{}Down{}", style::Bold, style::Reset));
        strings.push("".into());

        for cell in &self.grid {
            if let Some(ref clue) = cell.clue_down {
                let mut tmp = format!("{}. {}", cell.clue_number.unwrap(), clue);
                tmp.truncate(clues_width as usize);

                match cursor_clue_number {
                    Some(n) if n == cell.clue_number.unwrap() => {
                        strings.push(format!("{}{}{}", style::Bold, tmp, style::Reset));
                    }
                    _ => strings.push(tmp),
                }
            }
        }

        for i in 0..clues_height {
            write!(
                self.stdout,
                "{}{}",
                cursor::Goto(self.width * 4 + 3, i as u16 + 1),
                clear::UntilNewline
            ).unwrap();
        }

        for (i, string) in strings
            .iter()
            .skip(self.clues_scroll as usize)
            .take(clues_height as usize)
            .enumerate()
        {
            write!(
                self.stdout,
                "{}",
                cursor::Goto(self.width * 4 + 3, i as u16 + 1)
            ).unwrap();
            write!(self.stdout, "{}", string).unwrap();
        }
    }

    fn draw_cursor(&mut self) {
        write!(
            self.stdout,
            "{}",
            cursor::Goto(self.cursor_x * 4 + 2, self.cursor_y * 3 + 2)
        ).unwrap();
    }

    /// Calculate the y coordinate of the cell "above" a given y coordinate.
    ///
    /// This wraps when _y = 0_.
    fn up(&self, y: u16) -> u16 {
        if y == 0 {
            // Upper bound reached. Wrap around.
            self.height - 1
        } else {
            y - 1
        }
    }

    /// Calculate the y coordinate of the cell "below" a given y coordinate.
    ///
    /// This wraps when _y = h - 1_.
    fn down(&self, y: u16) -> u16 {
        if y + 1 == self.height {
            // Lower bound reached. Wrap around.
            0
        } else {
            y + 1
        }
    }

    /// Calculate the x coordinate of the cell "left to" a given x coordinate.
    ///
    /// This wraps when _x = 0_.
    fn left(&self, x: u16) -> u16 {
        if x == 0 {
            // Lower bound reached. Wrap around.
            self.width - 1
        } else {
            x - 1
        }
    }

    /// Calculate the x coordinate of the cell "left to" a given x coordinate.
    ///
    /// This wraps when _x = w - 1_.
    fn right(&self, x: u16) -> u16 {
        if x + 1 == self.width {
            // Upper bound reached. Wrap around.
            0
        } else {
            x + 1
        }
    }

    fn edit_left(&self, x: u16, y: u16) -> u16 {
        if x == 0 {
            return x;
        }

        match self.get(x - 1, y).truth {
            Some(_) => x - 1,
            _ => x,
        }
    }

    fn edit_right(&self, x: u16, y: u16) -> u16 {
        if x + 1 == self.width {
            return x;
        }

        match self.get(x + 1, y).truth {
            Some(_) => x + 1,
            _ => x,
        }
    }

    fn edit_up(&self, x: u16, y: u16) -> u16 {
        if y == 0 {
            return y;
        }

        match self.get(x, y - 1).truth {
            Some(_) => y - 1,
            _ => y,
        }
    }

    fn edit_down(&self, x: u16, y: u16) -> u16 {
        if y + 1 == self.height {
            return y;
        }

        match self.get(x, y + 1).truth {
            Some(_) => y + 1,
            _ => y,
        }
    }

    fn move_up(&mut self) {
        self.cursor_y = self.up(self.cursor_y);
        self.draw_clues();
    }

    fn move_down(&mut self) {
        self.cursor_y = self.down(self.cursor_y);
        self.draw_clues();
    }

    fn move_left(&mut self) {
        self.cursor_x = self.left(self.cursor_x);
        self.draw_clues();
    }

    fn move_right(&mut self) {
        self.cursor_x = self.right(self.cursor_x);
        self.draw_clues();
    }

    fn edit_move_up(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.cursor_y = self.edit_up(self.cursor_x, self.cursor_y);

        self.draw_cell(x, y);
        self.draw_cursor_cell();
    }

    fn edit_move_down(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.cursor_y = self.edit_down(self.cursor_x, self.cursor_y);

        self.draw_cell(x, y);
        self.draw_cursor_cell();
    }

    fn edit_move_left(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.cursor_x = self.edit_left(self.cursor_x, self.cursor_y);

        self.draw_cell(x, y);
        self.draw_cursor_cell();
    }

    fn edit_move_right(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.cursor_x = self.edit_right(self.cursor_x, self.cursor_y);

        self.draw_cell(x, y);
        self.draw_cursor_cell();
    }

    /// Enter an appropriate edit mode for the current cursor position.
    fn edit_mode(&mut self) {
        // Can't edit a black cell
        if self.get(self.cursor_x, self.cursor_y).truth.is_none() {
            return;
        }

        self.mode = match (
            self.get(self.cursor_x, self.cursor_y).clue_across.as_ref(),
            self.get(self.cursor_x, self.cursor_y).clue_down.as_ref(),
        ) {
            (Some(_), None) => Mode::EditAcross,
            (None, Some(_)) => Mode::EditDown,
            _ => self.last_edit_mode,
        };

        self.last_edit_mode = self.mode;

        self.draw_cursor_cell();
    }

    /// Change edit direction between across and down
    fn edit_direction(&mut self) {
        self.mode = match self.mode {
            Mode::EditDown => Mode::EditAcross,
            _ => Mode::EditDown,
        };

        self.last_edit_mode = self.mode;

        self.draw_cursor_cell();
    }

    /// Enter select mode
    fn select_mode(&mut self) {
        self.mode = Mode::Select;

        self.draw_cursor_cell();
    }

    /// Put a guess into the current cell
    fn input(&mut self, c: char) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        // This doesn't work for certain characters, and I'm okay with that.

        let upper = c.to_uppercase().collect::<Vec<_>>().swap_remove(0);

        self.get_mut(x, y).guess = Some(upper);

        self.next();
    }

    /// Removes the guess at the current cell
    fn unguess(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        self.get_mut(x, y).guess = None;
        self.draw_cursor_cell();
        self.draw_status_bar();
    }

    /// Move the cursor to the next cell to be edited
    fn next(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        match self.mode {
            Mode::EditAcross => self.edit_move_right(),
            Mode::EditDown => self.edit_move_down(),
            _ => {}
        }

        self.draw_cell(x, y);
        self.draw_cursor_cell();
        self.draw_status_bar();
    }

    /// Move the cursor to the previous cell to be edited
    fn prev(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        match self.mode {
            Mode::EditAcross => self.edit_move_left(),
            Mode::EditDown => self.edit_move_up(),
            _ => {}
        }

        self.draw_cell(x, y);
        self.draw_cursor_cell();
        self.draw_status_bar();
    }

    fn clues_scroll_up(&mut self) {
        if self.clues_scroll <= 5 {
            self.clues_scroll = 0;
        } else {
            self.clues_scroll -= 5;
        }

        self.draw_clues();
    }

    fn clues_scroll_down(&mut self) {
        self.clues_scroll += 5;

        self.draw_clues();
    }

    fn pause(&mut self) {
        self.mode = Mode::Pause;

        write!(self.stdout, "{}", clear::All).unwrap();

        let (term_width, term_height) = termion::terminal_size().unwrap();

        let msg = "Game Paused.";

        write!(
            self.stdout,
            "{}{}{}{}",
            cursor::Goto((term_width - msg.len() as u16) / 2, term_height / 2 - 1),
            style::Bold,
            msg,
            style::Reset
        ).unwrap();

        let msg = "Press p to continue.";

        write!(
            self.stdout,
            "{}{}",
            cursor::Goto((term_width - msg.len() as u16) / 2, term_height / 2),
            msg
        ).unwrap();

        let msg = "Press q to quit.";

        write!(
            self.stdout,
            "{}{}",
            cursor::Goto((term_width - msg.len() as u16) / 2, term_height / 2 + 1),
            msg
        ).unwrap();

        self.draw_status_bar();
        self.stdout.flush().unwrap();

        self.stopwatch.stop();
    }

    fn unpause(&mut self) {
        self.mode = Mode::Select;

        write!(self.stdout, "{}", clear::All).unwrap();

        self.draw_all();

        self.stopwatch.start();
    }

    fn start(&mut self) {
        self.stopwatch.start();

        loop {
            self.tick += 1;

            if !self.update() {
                break;
            }

            if self.tick % 10 == 0 {
                self.draw_status_bar();
                self.draw_cursor();
                self.stdout.flush().unwrap();
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn update(&mut self) -> bool {
        while let Some(b) = self.stdin.next() {
            if let Ok(c) = b {
                use termion::event::Key::*;

                match self.mode {
                    Mode::Pause => {
                        match c {
                            Char('p') | Char('\n') | Esc => self.unpause(),
                            Char('q') | Ctrl('c') => return false,
                            _ => {}
                        }
                    }
                    Mode::Select => {
                        match c {
                            PageUp => self.clues_scroll_up(),
                            PageDown => self.clues_scroll_down(),
                            Char('h') | Char('a') | Left => self.move_left(),
                            Char('j') | Char('s') | Down => self.move_down(),
                            Char('k') | Char('w') | Up => self.move_up(),
                            Char('l') | Char('d') | Right => self.move_right(),
                            Char('q') | Char('p') | Ctrl('c') | Esc => self.pause(),
                            Char('\n') | Char('i') => self.edit_mode(),
                            _ => {} 
                        }
                    }
                    Mode::EditAcross | Mode::EditDown => {
                        match c {
                            Delete => self.unguess(),
                            PageUp => self.clues_scroll_up(),
                            PageDown => self.clues_scroll_down(),
                            Backspace => self.prev(), 
                            Left => self.edit_move_left(),
                            Down => self.edit_move_down(),
                            Up => self.edit_move_up(),
                            Right => self.edit_move_right(),
                            Char('\n') | Esc => self.select_mode(),
                            Char(' ') => self.edit_direction(),
                            Ctrl('c') => return false,
                            Char(c) if c.is_alphabetic() => {
                                self.input(c);
                            }
                            _ => {} 
                        }
                    }
                }

                self.draw_cursor();
                self.stdout.flush().unwrap();
            }
        }

        true
    }
}

fn main() {
    let filename = env::args().nth(1).expect("Please specify a puzzle file.");

    let mut f = File::open(&Path::new(&filename)).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();

    let p = match puzfile::parse_all(&v[..]) {
        nom::IResult::Done(_, p) => p,
        nom::IResult::Incomplete(x) => panic!("incomplete: {:?}", x),
        nom::IResult::Error(e) => panic!("error: {:?}", e),
    };

    let stdout = io::stdout();
    let stdout = stdout.lock();
    let stdout = stdout.into_raw_mode().unwrap();

    let stdin = async_stdin().keys();

    init(stdin, stdout, &p);
}

#[macro_use]
extern crate nom;
extern crate encoding;
extern crate stopwatch;
extern crate termion;

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{async_stdin, clear, color, cursor, style};

use stopwatch::Stopwatch;

use nom::Err;

mod puzfile;

use puzfile::PuzFile;

#[derive(Copy, Clone)]
enum Mode {
    EditAcross,
    EditDown,
    GameOver,
    Select,
    Pause,
}

#[derive(Copy, Clone)]
enum Direction {
    Down,
    Left,
    Right,
    Up,
}

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
    cursor_x: u16,
    cursor_y: u16,
    clues_scroll: u16,
    mode: Mode,
    last_edit_mode: Mode,
    stdout: W,
    stdin: R,
    stopwatch: Stopwatch,
    tick: u64,
    version: &'static str,
    hint_num_errors: bool,
    title: String,
    author: String,
}

pub struct GameStatus {
    cells: u16,
    guesses: u16,
    errors: u16,
}

fn init<W: Write, R: Read>(stdin: R, mut stdout: W, p: &PuzFile) {
    let mut grid = Vec::new();

    for c in p.puzzle.chars() {
        let truth = match c {
            '.' => None,
            _ => Some(c),
        };

        grid.push(Cell {
            truth,
            guess: None,
            clue_number: None,
            clue_across: None, // TODO
            clue_down: None,   // TODO
        });
    }

    write!(stdout, "{}", clear::All).unwrap();

    let mut g = Game {
        width: u16::from(p.width),
        height: u16::from(p.height),
        grid,
        cursor_x: 0,
        cursor_y: 0,
        clues_scroll: 0,
        mode: Mode::Select,
        last_edit_mode: Mode::EditAcross,
        stdout,
        stdin: stdin.keys(),
        stopwatch: Stopwatch::new(),
        tick: 0,
        version: env!("CARGO_PKG_VERSION"),
        hint_num_errors: false,
        title: p.title.clone(),
        author: p.author.clone(),
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

impl<R, W: Write> Drop for Game<R, W> {
    fn drop(&mut self) {
        // When done, restore the defaults to avoid messing with the terminal.
        write!(
            self.stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1)
        )
        .unwrap();
    }
}

impl<R: Iterator<Item = Result<Key, std::io::Error>>, W: Write> Game<R, W> {
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

        // If the cell left is black or out of bounds, and there's at least one
        // non-black cell right.

        if (x == 0 || self.get(x - 1, y).truth.is_none())
            && x < self.width - 1
            && self.get(x + 1, y).truth.is_some()
        {
            return true;
        }

        false
    }

    fn has_clue_down(&self, x: u16, y: u16) -> bool {
        if self.get(x, y).truth.is_none() {
            return false;
        }

        // If the cell above is black or out of bounds, and there's at least one
        // non-black cell below.

        if (y == 0 || self.get(x, y - 1).truth.is_none())
            && y < self.height - 1
            && self.get(x, y + 1).truth.is_some()
        {
            return true;
        }

        false
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

    fn is_game_over(&self) -> bool {
        let status = self.get_status();

        status.errors == 0 && status.cells == status.guesses
    }

    fn draw_cell(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 1)).unwrap();

        let cross = match (x == self.width - 1, y == self.height - 1) {
            (true, true) => "\u{251b}",
            (true, false) => "\u{252b}",
            (false, true) => "\u{253b}",
            (false, false) => "\u{254b}",
        };

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
                    Some(g) => write!(
                        self.stdout,
                        " {}{}{} {}",
                        style::Bold,
                        g,
                        style::Reset,
                        right_border
                    )
                    .unwrap(),
                    None => write!(self.stdout, "   {}", right_border).unwrap(),
                };
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();

                // Draw a downward-pointing arrow in the bottom border if this is the
                // selected cell and we're in Mode::EditDown

                match self.mode {
                    Mode::EditDown if self.cursor_x == x && self.cursor_y == y => write!(
                        self.stdout,
                        "\u{2501}{}\u{25BC}{}\u{2501}\u{254B}",
                        color::Fg(color::LightRed),
                        style::Reset
                    )
                    .unwrap(),
                    _ => write!(self.stdout, "\u{2501}\u{2501}\u{2501}{}", cross).unwrap(),
                }
            }
            None => {
                // Draw a black cell

                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 2)).unwrap();
                write!(self.stdout, "\u{2588}\u{2588}\u{2588}\u{2503}").unwrap();
                write!(self.stdout, "{}", cursor::Goto(x * 4 + 1, y * 3 + 3)).unwrap();
                write!(self.stdout, "\u{2501}\u{2501}\u{2501}{}", cross).unwrap();
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
        )
        .unwrap();

        write!(
            self.stdout,
            "puzterm {} G{}/{} E{} T{}:{:02}:{:02}",
            self.version,
            s.guesses,
            s.cells,
            if self.hint_num_errors {
                s.errors.to_string()
            } else {
                "?".to_string()
            },
            self.stopwatch.elapsed().as_secs() / 60 / 60,
            (self.stopwatch.elapsed().as_secs() / 60) % 60,
            self.stopwatch.elapsed().as_secs() % 60,
        )
        .unwrap();

        write!(self.stdout, "{}", style::Reset).unwrap();
    }

    fn draw_all(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.draw_cell(x, y);
            }
        }

        self.draw_title();
        self.draw_clues();
        self.draw_status_bar();
        self.draw_cursor();

        self.stdout.flush().unwrap();
    }

    fn draw_title(&mut self) {
        write!(
            self.stdout,
            "{}{:.width$}{}{:.width$}",
            cursor::Goto(0, self.height * 3 + 1),
            self.title,
            cursor::Goto(0, self.height * 3 + 2),
            self.author,
            width = self.width as usize * 4
        )
        .unwrap();
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
            )
            .unwrap();
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
            )
            .unwrap();
            write!(self.stdout, "{}", string).unwrap();
        }
    }

    fn draw_cursor(&mut self) {
        write!(
            self.stdout,
            "{}",
            cursor::Goto(self.cursor_x * 4 + 2, self.cursor_y * 3 + 2)
        )
        .unwrap();
    }

    fn draw_message_screen(&mut self, messages: &[String]) {
        write!(self.stdout, "{}", clear::All).unwrap();

        let (term_width, term_height) = termion::terminal_size().unwrap();

        let height = messages.len() as u16;

        for (i, message) in messages.iter().enumerate() {
            if i == 0 {
                write!(self.stdout, "{}", style::Bold).unwrap();
            }

            write!(
                self.stdout,
                "{}{}",
                cursor::Goto(
                    (term_width - message.len() as u16) / 2,
                    term_height / 2 - height / 2 + i as u16,
                ),
                message
            )
            .unwrap();

            if i == 0 {
                write!(self.stdout, "{}", style::Reset).unwrap();
            }
        }
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

    /// Calculate the x coordinate of the cell "right to" a given x coordinate.
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

    /// Calculate the x coordinate of the cell "left to" given x, y coordinates.
    ///
    /// This does not wrap, and stops at black cells.
    fn edit_left(&self, x: u16, y: u16) -> u16 {
        if x == 0 {
            return x;
        }

        match self.get(x - 1, y).truth {
            Some(_) => x - 1,
            _ => x,
        }
    }

    /// Calculate the x coordinate of the cell "right to" given x, y coordinates.
    ///
    /// This does not wrap, and stops at black cells.
    fn edit_right(&self, x: u16, y: u16) -> u16 {
        if x + 1 == self.width {
            return x;
        }

        match self.get(x + 1, y).truth {
            Some(_) => x + 1,
            _ => x,
        }
    }

    /// Calculate the y coordinate of the cell "above" given x, y coordinates.
    ///
    /// This does not wrap, and stops at black cells.
    fn edit_up(&self, x: u16, y: u16) -> u16 {
        if y == 0 {
            return y;
        }

        match self.get(x, y - 1).truth {
            Some(_) => y - 1,
            _ => y,
        }
    }

    /// Calculate the y coordinate of the cell "below" given x, y coordinates.
    ///
    /// This does not wrap, and stops at black cells.
    fn edit_down(&self, x: u16, y: u16) -> u16 {
        if y + 1 == self.height {
            return y;
        }

        match self.get(x, y + 1).truth {
            Some(_) => y + 1,
            _ => y,
        }
    }

    fn select_move(&mut self, direction: Direction) {
        match direction {
            Direction::Up => {
                self.cursor_y = self.up(self.cursor_y);
            }
            Direction::Down => {
                self.cursor_y = self.down(self.cursor_y);
            }
            Direction::Left => {
                self.cursor_x = self.left(self.cursor_x);
            }
            Direction::Right => {
                self.cursor_x = self.right(self.cursor_x);
            }
        }

        self.draw_clues();
    }

    fn edit_move(&mut self, direction: Direction) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        match direction {
            Direction::Up => {
                self.cursor_y = self.edit_up(self.cursor_x, self.cursor_y);
            }
            Direction::Down => {
                self.cursor_y = self.edit_down(self.cursor_x, self.cursor_y);
            }
            Direction::Left => {
                self.cursor_x = self.edit_left(self.cursor_x, self.cursor_y);
            }
            Direction::Right => {
                self.cursor_x = self.edit_right(self.cursor_x, self.cursor_y);
            }
        }

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

    /// Enter game over mode
    fn game_over_mode(&mut self) {
        self.mode = Mode::GameOver;

        self.draw_message_screen(&[
            "Game Over.".into(),
            "".into(),
            "Press any key to quit.".into(),
        ]);

        self.draw_status_bar();
        self.stdout.flush().unwrap();

        self.stopwatch.stop();
    }

    /// Put a guess into the current cell
    fn input(&mut self, c: char) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        // This doesn't work for certain characters, and I'm okay with that.

        let upper = c.to_uppercase().collect::<Vec<_>>().swap_remove(0);

        self.get_mut(x, y).guess = Some(upper);

        self.edit_next();
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
    fn edit_next(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        match self.mode {
            Mode::EditAcross => self.edit_move(Direction::Right),
            Mode::EditDown => self.edit_move(Direction::Down),
            _ => {}
        }

        self.draw_cell(x, y);
        self.draw_cursor_cell();
        self.draw_status_bar();
    }

    /// Move the cursor to the previous cell to be edited
    fn edit_prev(&mut self) {
        let x = self.cursor_x;
        let y = self.cursor_y;

        match self.mode {
            Mode::EditAcross => self.edit_move(Direction::Left),
            Mode::EditDown => self.edit_move(Direction::Up),
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

        self.draw_message_screen(&[
            "Game Paused".into(),
            "".into(),
            "Press p to continue.".into(),
            "Press q to quit.".into(),
        ]);

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

    fn toggle_hint_num_errors(&mut self) {
        self.hint_num_errors = !self.hint_num_errors;

        self.draw_status_bar();
        self.stdout.flush().unwrap();
    }

    fn start(&mut self) {
        self.stopwatch.start();

        loop {
            self.tick += 1;

            if !self.update() {
                break;
            }

            match self.mode {
                Mode::GameOver => {}
                _ => {
                    if self.is_game_over() {
                        self.game_over_mode();
                    }
                }
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
                    Mode::Pause => match c {
                        Char('p') | Char('\n') | Esc => self.unpause(),
                        Char('q') | Ctrl('c') => return false,
                        _ => {}
                    },
                    Mode::Select => match c {
                        PageUp => self.clues_scroll_up(),
                        PageDown => self.clues_scroll_down(),
                        Char('h') | Char('a') | Left => self.select_move(Direction::Left),
                        Char('j') | Char('s') | Down => self.select_move(Direction::Down),
                        Char('k') | Char('w') | Up => self.select_move(Direction::Up),
                        Char('l') | Char('d') | Right => self.select_move(Direction::Right),
                        Char('q') | Char('p') | Ctrl('c') | Esc => self.pause(),
                        Char('e') => self.toggle_hint_num_errors(),
                        Char('\n') | Char('i') => self.edit_mode(),
                        _ => {}
                    },
                    Mode::EditAcross | Mode::EditDown => match c {
                        Delete => self.unguess(),
                        PageUp => self.clues_scroll_up(),
                        PageDown => self.clues_scroll_down(),
                        Backspace => self.edit_prev(),
                        Left => self.edit_move(Direction::Left),
                        Down => self.edit_move(Direction::Down),
                        Up => self.edit_move(Direction::Up),
                        Right => self.edit_move(Direction::Right),
                        Char('\n') | Esc => self.select_mode(),
                        Char(' ') => self.edit_direction(),
                        Char(c) if c.is_alphanumeric() => {
                            self.input(c);
                        }
                        _ => {}
                    },
                    Mode::GameOver => return false,
                }

                self.draw_cursor();
                self.stdout.flush().unwrap();
            }
        }

        true
    }
}

fn main() {
    let filename = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: puzterm <file>");
        ::std::process::exit(1);
    });

    let mut f = File::open(&Path::new(&filename)).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();

    let p = match puzfile::parse_all(&v[..]) {
        Ok((_, p)) => p,
        Err(Err::Incomplete(x)) => panic!("incomplete: {:?}", x),
        Err(Err::Error(e)) => panic!("error: {:?}", e),
        Err(Err::Failure(e)) => panic!("failure: {:?}", e),
    };

    let stdout = io::stdout();
    let stdout = stdout.lock();
    let stdout = stdout.into_raw_mode().unwrap();

    let stdin = async_stdin();

    init(stdin, stdout, &p);
}

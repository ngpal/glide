use crossterm::{
    cursor::{position, MoveTo, MoveToColumn, MoveToNextLine, MoveToRow, MoveUp},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Print, PrintStyledContent, Stylize},
    terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType},
};
use std::{
    io::{self, stdout, Write},
    time::Duration,
};

#[derive(Default)]
#[allow(unused)]
pub struct Repl {
    users: Vec<String>,
    buffer: String,
    cursor_pos: u16,
    cur_input_row: u16,
    quit: bool,
}

impl Drop for Repl {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        println!("\nBye.");
    }
}

impl Repl {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self::default())
    }

    fn increment_cursor_pos(&mut self, n: u16) {
        if (self.cursor_pos as usize) < self.buffer.len() {
            self.cursor_pos = self.cursor_pos.saturating_add(n);
        }
    }

    fn decrement_cursor_pos(&mut self, n: u16) {
        self.cursor_pos = self.cursor_pos.saturating_sub(n);
    }

    fn set_cur_input_row(&mut self) -> io::Result<()> {
        self.cur_input_row = position()?.1;
        Ok(())
    }

    fn get_output_rows(&self, output: &str) -> io::Result<u16> {
        Ok(output.len() as u16 / size()?.1 + u16::from(output.len() as u16 % size()?.1 != 0))
    }

    pub fn run(&mut self) -> io::Result<()> {
        execute!(stdout(), PrintStyledContent("> ".bold().blue()))?;
        self.set_cur_input_row()?;

        loop {
            if !poll(Duration::from_millis(10))? {
                continue;
            }

            match read()? {
                Event::Key(event) => {
                    self.handle_key_event(event)?;
                }
                _ => continue,
            };

            stdout().flush()?;

            if self.quit {
                break;
            }
        }

        Ok(())
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) -> io::Result<()> {
        match (event.code, event.modifiers) {
            // Keyboard shortcuts
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.quit = true,
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                queue!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
                self.cur_input_row = 0;
            }

            // Input
            (KeyCode::Char(ch), _) => {
                if size()?.1 - 1 == position()?.1 {
                    queue!(stdout(), Print("\n"), MoveUp(1))?;
                    self.cur_input_row -= 1;
                }

                self.buffer.insert(self.cursor_pos.into(), ch);
                self.increment_cursor_pos(1);
            }
            (KeyCode::Backspace, _) => {
                if !self.buffer.is_empty() && self.cursor_pos != 0 {
                    self.buffer.remove(self.cursor_pos as usize - 1);
                    self.decrement_cursor_pos(1);
                }
            }

            // Process the contents of the buffer and clear when enter is hit
            (KeyCode::Enter, _) => {
                let output = self.process_buffer();

                // Check if we're on the last line, extend by two
                if size()?.1 - 2 >= position()?.1 {
                    // This is horrible code, please forgive me until I figure something out
                    let clear_height =
                        self.get_output_rows(&output.clone().unwrap_or_else(|x| x))? + 1;

                    queue!(
                        stdout(),
                        Print("\n".repeat(clear_height.into())),
                        MoveUp(clear_height)
                    )?;
                }

                if self.buffer.trim().is_empty() {
                    queue!(stdout(), MoveToNextLine(1))?;
                    return Ok(());
                }

                match output {
                    Ok(output) => queue!(
                        stdout(),
                        MoveToNextLine(1),
                        Print(output),
                        MoveToNextLine(1)
                    )?,
                    Err(err) => queue!(
                        stdout(),
                        MoveToNextLine(1),
                        PrintStyledContent("ERROR".bold().red()),
                        Print(format!(": {}", err)),
                        MoveToNextLine(1),
                    )?,
                };

                self.buffer.clear();
                self.cursor_pos = 0;
                self.set_cur_input_row()?;
            }

            // Handle arrow keys
            (KeyCode::Left, _) => self.decrement_cursor_pos(1),
            (KeyCode::Right, _) => self.increment_cursor_pos(1),
            _ => {}
        }

        self.update_text()?;
        Ok(())
    }

    fn update_text(&mut self) -> io::Result<()> {
        let (cols, _) = size()?;
        queue!(
            stdout(),
            MoveToRow(self.cur_input_row),
            Clear(ClearType::CurrentLine),
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            PrintStyledContent("> ".bold().blue()),
            Print(&self.buffer),
            MoveToColumn((2 + self.cursor_pos) % cols),
            MoveToRow(self.cur_input_row + (2 + self.cursor_pos) / cols),
        )?;
        Ok(())
    }

    fn process_buffer(&self) -> Result<String, String> {
        match self.buffer.trim() {
            "error" => Err("This is a big bad error!".into()),
            _ => Ok(self.buffer.clone()),
        }
    }
}

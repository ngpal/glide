use crossterm::{
    cursor::{position, MoveToColumn, MoveToNextLine, MoveUp},
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
        return Ok(Self::default());
    }

    fn increment_cursor_pos(&mut self, n: u16) {
        if (self.cursor_pos as usize) < self.buffer.len() {
            self.cursor_pos = self.cursor_pos.saturating_add(n);
        }
    }

    fn decrement_cursor_pos(&mut self, n: u16) {
        self.cursor_pos = self.cursor_pos.saturating_sub(n);
    }

    pub fn run(&mut self) -> io::Result<()> {
        execute!(stdout(), PrintStyledContent("> ".bold().blue()))?;
        loop {
            if !poll(Duration::from_millis(10))? {
                continue;
            }

            match read()? {
                Event::Key(event) => match self.handle_key_event(event)? {
                    true => break,
                    false => (),
                },
                _ => continue,
            };

            stdout().flush()?;
        }

        Ok(())
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) -> io::Result<bool> {
        // Returns true if the code should exit
        if let KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } = event
        {
            return Ok(true);
        }

        match event.code {
            KeyCode::Char(ch) => {
                self.buffer.insert(self.cursor_pos.into(), ch);
                self.increment_cursor_pos(1);
            }
            KeyCode::Backspace => {
                if !self.buffer.is_empty() {
                    self.buffer.remove(self.cursor_pos as usize - 1);
                    self.decrement_cursor_pos(1);
                }
            }

            // Process the contents of the buffer and clear when enter is hit
            KeyCode::Enter => {
                // Check if we're on the last line, extend by two
                if size()?.1 - 1 == position()?.1 {
                    queue!(stdout(), Print("\n\n"), MoveUp(2))?;
                }

                match self.process_buffer() {
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
            }

            // Handle arrow keys
            KeyCode::Left => self.decrement_cursor_pos(1),
            KeyCode::Right => self.increment_cursor_pos(1),
            _ => return Ok(false),
        }

        self.update_text()?;
        Ok(false)
    }

    fn update_text(&mut self) -> io::Result<()> {
        queue!(
            stdout(),
            Clear(ClearType::CurrentLine),
            MoveToColumn(0),
            PrintStyledContent("> ".bold().blue()),
            Print(&self.buffer),
            MoveToColumn(2 + self.cursor_pos)
        )?;
        Ok(())
    }

    fn process_buffer(&self) -> Result<String, String> {
        match &*self.buffer {
            "error" => Err("This is a big bad error!".into()),
            _ => Ok(self.buffer.clone()),
        }
    }
}

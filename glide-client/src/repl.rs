#![allow(unused)]

use crossterm::{
    cursor::MoveToColumn,
    event::{read, Event, KeyCode, KeyEvent},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::io::{self, stdout, Stdout};

#[derive(Default)]
pub struct Repl {
    users: Vec<String>,
    buffer: String,
}

impl Repl {
    pub fn run(&mut self) -> io::Result<()> {
        loop {
            match read()? {
                Event::Key(event) => match self.handle_key_event(event)? {
                    true => break,
                    false => (),
                },
                _ => continue,
            };
        }

        Ok(())
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) -> io::Result<bool> {
        // Returns true if the code should exit
        match event.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char(ch) => {
                self.buffer.push(ch);
                self.update_text()?;
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                stdout().execute(Clear(ClearType::CurrentLine))?;
                self.update_text()?;
            }
            _ => return Ok(false),
        }

        Ok(false)
    }

    pub fn update_text(&mut self) -> io::Result<()> {
        execute!(stdout(), MoveToColumn(0), Print(&self.buffer))?;
        Ok(())
    }

    pub fn process_command(cmd: String) -> Result<String, String> {
        Ok(cmd)
    }
}

use crossterm::{
    self,
    event::{read, Event, KeyCode},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, stdout};

fn main() -> io::Result<()> {
    enable_raw_mode()?;

    loop {
        match read()? {
            Event::Key(event) => match event.code {
                KeyCode::Char('q') => {
                    break;
                }
                KeyCode::Char(ch) => execute!(stdout(), Print(ch))?,
                _ => continue,
            },
            _ => continue,
        };
    }

    disable_raw_mode()?;
    println!();
    Ok(())
}

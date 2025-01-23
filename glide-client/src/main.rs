mod repl;
use crossterm::{
    self,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use repl::Repl;
use std::io;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut repl = Repl::default();
    repl.run()?;

    disable_raw_mode()?;
    println!();
    Ok(())
}

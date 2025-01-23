mod repl;

use repl::Repl;
use std::io;

fn main() -> io::Result<()> {
    let mut repl = Repl::new()?;
    repl.run()?;

    Ok(())
}

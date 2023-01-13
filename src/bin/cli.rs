#![cfg(feature = "cli")]
#![feature(map_try_insert)]

use damasc::{parser::statement, repl::ReplError};
use rustyline::error::ReadlineError;
use rustyline::Editor;

const INITIAL_BAG_NAME: &str = "init";

pub(crate) fn main() -> rustyline::Result<()> {
    let mut repl = damasc::repl::Repl::new(INITIAL_BAG_NAME);
    let mut rl = Editor::<()>::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }

    println!("Welcome");
    println!("press CTRL-D to exit.");
    println!(".bag");
    println!("Current Bag: {}", repl.current_bag);

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                let input = line.as_str();

                let stmt = match statement(input) {
                    Ok((_, s)) => s,
                    Err(e) => {
                        println!("read error: {e}");
                        continue;
                    }
                };

                match repl.execute(stmt) {
                    Ok(r) => {
                        println!("{r}")
                    }
                    Err(ReplError::Exit) => break,
                    Err(e) => println!("Error: {e:?}"),
                }
            }
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        }
    }
    rl.save_history("history.txt")
}

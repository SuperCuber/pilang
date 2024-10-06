#![allow(dead_code)] // TODO
#![allow(unused_imports)]

use anyhow::Result;
use interpreter::Interpreter;
use std::io::{stdin, BufRead, Write};

mod data;
mod interpreter;
mod parser;

fn main() -> Result<()> {
    run_prompt()
}

fn run_prompt() -> Result<()> {
    let mut interpreter = Interpreter::new();

    let stdin = stdin();
    let stdin = stdin.lock();
    print!("> ");
    std::io::stdout().flush().unwrap();
    for line in stdin.lines() {
        if let Ok(line) = line {
            if let Err(err) = run(line, &mut interpreter) {
                eprintln!("{}", err);
            };
            print!("> ");
            std::io::stdout().flush().unwrap();
        } else {
            println!("End of input. Goodbye!");
            break;
        }
    }
    Ok(())
}
fn run(line: String, interpreter: &mut Interpreter) -> Result<()> {
    let command = parser::parse_command(&line)?;
    dbg!(command);
    Ok(())
}

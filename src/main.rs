// TODO
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use anyhow::Result;
use interpreter::Interpreter;
use std::io::{stdin, stdout, BufRead, Write};

mod data;
mod interpreter;
mod parser;

fn main() -> Result<()> {
    run_prompt()
}

fn run_prompt() -> Result<()> {
    let mut interpreter = Interpreter::new("".into());

    let stdin = stdin();
    let stdin = stdin.lock();
    print!("> ");
    stdout().flush().unwrap();
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
    let input = parser::user_input(&line)?;
    match input {
        parser::UserInput::Command(command) => interpreter.run(command),
        parser::UserInput::Directive(_, _) => todo!(),
    }
    interpreter.show_sample();
    Ok(())
}

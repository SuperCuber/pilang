// TODO
#![allow(dead_code)]
#![allow(unused_variables)]

use anyhow::{Context, Result};
use interpreter::Interpreter;
use std::io::{stdin, stdout, BufRead, Write};

mod builtin;
mod data;
mod error;
mod interpreter;
mod parser;

fn main() -> Result<()> {
    run_prompt()
}

fn run_prompt() -> Result<()> {
    let mut interpreter =
        Interpreter::new("{\"a\": 1, \"b\": 2.0, \"c\": [1,2,3], \"d\": null}".into());

    let stdin = stdin();
    let stdin = stdin.lock();
    prompt(&interpreter);
    for line in stdin.lines() {
        if let Ok(line) = line {
            match run(line, &mut interpreter) {
                Ok(true) => {}
                Ok(false) => break,
                Err(err) => eprintln!("Error: {:#?}", err),
            }
            prompt(&interpreter);
        } else {
            println!("End of input. Goodbye!");
            break;
        }
    }
    Ok(())
}

fn prompt(interpreter: &Interpreter) {
    let status = interpreter.status();
    let val = interpreter.value();
    if let Err(err) = val.sample() {
        eprintln!("Error: {:#?}", err);
    };
    println!("{}", status.join(" >> "));
    println!("{val}");
    print!("$> ");
    stdout().flush().unwrap();
}

fn run(line: String, interpreter: &mut Interpreter) -> Result<bool> {
    let input = parser::user_input(&line)?;
    match input {
        parser::UserInput::Command(command) => {
            interpreter.run(command).context("running command")?
        }
        parser::UserInput::Directive(name, _) => match name.as_str() {
            "undo" | "u" => interpreter.undo(),
            "exit" | "quit" | "q" => return Ok(false),
            "done" | "d" => {
                let val = interpreter.value();
                if let Err(err) = val.realize() {
                    eprintln!("Error: {:#?}", err);
                };
                println!("{val}");
                return Ok(false);
            }
            _ => eprintln!("Unknown directive `{}`", name),
        },
    }
    Ok(true)
}

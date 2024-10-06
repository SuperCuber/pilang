use std::collections::HashMap;

use crate::data::{List, Map, Value};

peg::parser! {
  grammar pi_parser() for str {
    rule __()
      = [' ' | '\n' | '\t']+

    rule _()
      = __()?

    rule number() -> u32
      = n:$(['0'..='9']+) {? n.parse().or(Err("u32")) }

    rule string() -> String
      = "\"" s:$([^ '"']*) "\"" { s.to_string() }

    rule ident()
        = quiet!{[ 'a'..='z' | 'A'..='Z']['a'..='z' | 'A'..='Z' | '0'..='9' ]*}
        / expected!("identifier")

    rule keyword() -> Keyword
      = k:$(ident()) {? k.parse().or(Err("keyword")) }

    rule value() -> Value
      = n:number() { Value::Number(n) }
      / s:string() { Value::String(s.to_string()) }
      / "[" __ v:value() ** _ __ "]" { Value::List(List { elements: v, rest: None }) }
      / "{" __ pairs:(k:string() __ ":" __ v:value() {(k,v)}) ** _ __ "}" { Value::Map(Map { elements: pairs.into_iter().collect(), rest: None }) }

    pub rule command() -> Command
        = f:$(ident()) args:(_ a:value() ** _ {a})? {
            let c = if let Ok(k) = f.parse() {
                Callable::Keyword(k)
            } else {
                Callable::Function(f.to_string())
            };
            Command { callable: c, args: args.unwrap_or_default() }
        }
        / ">>" { Command { callable: Callable::Keyword(Keyword::ShiftRight), args: vec![] } }
        / "<<" { Command { callable: Callable::Keyword(Keyword::ShiftLeft), args: vec![] } }

    pub rule program() -> Vec<Command>
        = command() ** "\n"
  }
}
pub use pi_parser::command as parse_command;
pub use pi_parser::program as parse_program;

#[derive(Debug)]
pub enum Keyword {
    ShiftRight,
    ShiftLeft,
    Calc,
}

#[derive(Debug)]
pub enum Callable {
    Keyword(Keyword),
    Function(String),
}

#[derive(Debug)]
pub struct Command {
    pub callable: Callable,
    pub args: Vec<Value>,
}

impl std::str::FromStr for Keyword {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ">>" => Ok(Keyword::ShiftRight),
            "<<" => Ok(Keyword::ShiftLeft),
            "calc" => Ok(Keyword::Calc),
            _ => Err(()),
        }
    }
}

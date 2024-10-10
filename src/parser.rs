use std::collections::HashMap;

use crate::data::{SValue, Value};

peg::parser! {
  grammar pi_parser() for str {
    // Util
    rule _()
      = [' ' | '\n' | '\t']+

    rule ident()
        = quiet!{[ 'a'..='z' | 'A'..='Z']['a'..='z' | 'A'..='Z' | '0'..='9' ]*}
        / expected!("identifier")

    rule parens() -> Expression
        = "(" _? e:expression() _? ")" { e }

    rule number() -> u64
      = n:$(['0'..='9']+) {? n.parse().or(Err("u32")) }

    rule string() -> String
      = "\"" s:$([^ '"']*) "\"" { s.to_string() }

    rule literal() -> Value
        // TODO: float, null, bool
      = n:number() { Value::Int(n) }
      / s:string() { Value::String(s.to_string()) }

    rule list() -> Vec<Expression>
      = "[" _? v:expression() ** (_? "," _?) _? "]" { v }

    rule _pair() -> (String, Expression)
      = k:string() _? ":" _? v:expression() { (k, v) }

    rule dict() -> HashMap<String, Expression>
      = "{" _? pairs:(_pair() ** (_? "," _?)) _? "}" { pairs.into_iter().collect() }

    rule function_call() -> (String, Vec<Expression>)
      = f:$(ident()) args:(_ a:expression() ** _ {a})? { (f.to_string(), args.unwrap_or_default()) }

    rule expression() -> Expression
      = "%" { Expression::This }
      / p:parens() { p }
      / l:literal() { Expression::Literal(SValue::new(l)) }
      / l:list() { Expression::List(l) }
      / d:dict() { Expression::Dict(d) }
      / f:function_call() { Expression::FunctionCall(f.0, f.1) }

    pub rule command() -> Command
        = e:expression() { Command::Expression(e) }
        / ">>" kv:(_ k:$(ident()) _? ":" _? v:$(ident()) {(k,v)})? {
            Command::ShiftRight(kv.map(|(k,v)| (k.into(), v.into())))
        }
        / "<<" kv:(_ k:expression() _? ":" _? v:expression() {(k,v)})? {
            Command::ShiftLeft(kv)
        }

    pub rule user_input() -> UserInput
        = "." f:function_call() { UserInput::Directive(f.0, f.1) }
        / c:command() { UserInput::Command(c) }
  }
}
pub use pi_parser::*;

#[derive(Debug, PartialEq, Clone)]
pub enum Expression {
    This,
    Literal(SValue),
    List(Vec<Expression>),
    Dict(HashMap<String, Expression>),
    FunctionCall(String, Vec<Expression>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    /// The strings signify that we want to map over the string as actual pairs, bound to the following names
    ShiftRight(Option<(String, String)>),
    /// The expressions signify that we want to collect into a map, with the following pairs
    ShiftLeft(Option<(Expression, Expression)>),
    Expression(Expression),
}

#[derive(Debug, PartialEq)]
pub enum UserInput {
    Command(Command),
    Directive(String, Vec<Expression>),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(
            pi_parser::command("123"),
            Ok(Command::Expression(Expression::Literal(SValue::new(
                Value::Int(123)
            ))))
        );

        assert_eq!(
            pi_parser::command("\"hello\""),
            Ok(Command::Expression(Expression::Literal(SValue::new(
                Value::String("hello".to_string())
            ))))
        );

        assert_eq!(
            pi_parser::command("[1, 2  ,3]"),
            Ok(Command::Expression(Expression::List(vec![
                Expression::Literal(SValue::new(Value::Int(1))),
                Expression::Literal(SValue::new(Value::Int(2))),
                Expression::Literal(SValue::new(Value::Int(3))),
            ])))
        );

        assert_eq!(
            pi_parser::command("{ \"key\": 123 }"),
            Ok(Command::Expression(Expression::Dict(
                vec![(
                    "key".to_string(),
                    Expression::Literal(SValue::new(Value::Int(123)))
                )]
                .into_iter()
                .collect()
            )))
        );

        assert_eq!(
            pi_parser::command("%"),
            Ok(Command::Expression(Expression::This))
        );

        assert_eq!(
            pi_parser::command("get % (get 123)"),
            Ok(Command::Expression(Expression::FunctionCall(
                "get".to_string(),
                vec![
                    Expression::This,
                    Expression::FunctionCall(
                        "get".to_string(),
                        vec![Expression::Literal(SValue::new(Value::Int(123)))],
                    )
                ]
            )))
        );

        assert_eq!(
            pi_parser::command("print"),
            Ok(Command::Expression(Expression::FunctionCall(
                "print".to_string(),
                vec![]
            )))
        );

        assert_eq!(
            pi_parser::command("print 123"),
            Ok(Command::Expression(Expression::FunctionCall(
                "print".to_string(),
                vec![Expression::Literal(SValue::new(Value::Int(123)))]
            )))
        );

        assert_eq!(pi_parser::command(">>"), Ok(Command::ShiftRight(None)));

        assert_eq!(
            pi_parser::command(">> key:value"),
            Ok(Command::ShiftRight(Some((
                "key".to_string(),
                "value".to_string()
            ))))
        );

        assert_eq!(pi_parser::command("<<"), Ok(Command::ShiftLeft(None)));

        assert_eq!(
            pi_parser::command("<< \"test\": 1"),
            Ok(Command::ShiftLeft(Some((
                Expression::Literal(SValue::new(Value::String("test".to_string()))),
                Expression::Literal(SValue::new(Value::Int(1)))
            ))))
        );

        assert_eq!(
            pi_parser::user_input(".print"),
            Ok(UserInput::Directive("print".to_string(), vec![]))
        );

        assert_eq!(
            pi_parser::user_input(".print 123"),
            Ok(UserInput::Directive(
                "print".to_string(),
                vec![Expression::Literal(SValue::new(Value::Int(123)))]
            ))
        );
    }
}

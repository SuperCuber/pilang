use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::data::{Function, List, SValue, Value};
use crate::parser::{Command, Expression};
use crate::{builtin, error};

pub struct Interpreter {
    context: Rc<Context>,
    program: Program,
}

struct Context {
    functions: HashMap<String, Function>,
}

struct Program(SValue, Vec<ExecutedCommand>);

enum ExecutedCommand {
    /// A simple expression that replaces the current value
    Expression { expr: Expression, result: SValue },
    /// An incompelte shift-right that is in progress
    ShiftRight {
        name: String,
        kv: Option<(String, String)>,
        program: Program,
    },
    /// A finished shift-right that has replaced the current value
    ShiftLeft {
        kv: Option<(Expression, Expression)>,
        result: SValue,
    },
}

impl Interpreter {
    pub fn new(input: String) -> Self {
        Self {
            context: Rc::new(Context {
                functions: builtin::builtin_functions(),
            }),
            program: Program(SValue::new(Value::String(input)), vec![]),
        }
    }

    pub fn run(&mut self, command: Command) -> error::Result<()> {
        let this = self.value();
        match command.clone() {
            Command::Expression(e) => {
                let result = Interpreter::eval_expression(self.context.clone(), e.clone(), this)?;
                self.program
                    .1
                    .push(ExecutedCommand::Expression { expr: e, result });
            }
            Command::ShiftRight(kv) => match (&*this, kv) {
                (Value::List(l), None) => {
                    let name = "list".to_string();
                    let first = l.get(0)?.ok_or(error::Error::ShiftRightEmptySequence)?;
                    let program = Program(first, vec![]);
                    self.program.1.push(ExecutedCommand::ShiftRight {
                        name,
                        kv: None,
                        program,
                    });
                }
                (Value::Dict(_), None) => todo!(),
                (Value::Dict(_), Some(kv)) => todo!(),
                _ => todo!(),
            },
            Command::ShiftLeft(kv) => todo!(),
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        self.program.1.pop();
    }

    pub fn value(&self) -> SValue {
        self.program.value()
    }

    pub fn status(&self) -> Vec<String> {
        self.program.status()
    }

    fn eval_expression(context: Rc<Context>, e: Expression, this: SValue) -> error::Result<SValue> {
        Ok(match e {
            Expression::This => this.clone(),
            Expression::Literal(l) => l,
            Expression::List(l) => SValue::new(Value::List(List {
                elements: RefCell::new(vec![]),
                rest: RefCell::new(Some(Box::new(l.into_iter().map(move |e| {
                    Interpreter::eval_expression(context.clone(), e, this.clone())
                })))),
            })),
            Expression::Dict(_) => todo!(),
            Expression::FunctionCall(name, args) => {
                let Some(f) = context.functions.get(&name) else {
                    return Err(error::Error::FunctionNotFound(name));
                };
                let arity = args.len();

                let mut using_this = false;
                if !f.arities.contains(&arity) {
                    if f.arities.contains(&(arity + 1)) {
                        using_this = true;
                    } else {
                        return Err(error::Error::InvalidArity(name, arity, f.arities.clone()));
                    }
                }

                let prefix = if using_this {
                    Some(Expression::This)
                } else {
                    None
                };
                let args = prefix
                    .into_iter()
                    .chain(args.into_iter())
                    .map(|e| Interpreter::eval_expression(context.clone(), e, this.clone()))
                    .collect::<error::Result<Vec<_>>>()?;

                (f.implementation)(args)?
            }
        })
    }
}

impl Program {
    fn value(&self) -> SValue {
        if let Some(command) = self.1.last() {
            match command {
                ExecutedCommand::Expression { result, .. } => result.clone(),
                ExecutedCommand::ShiftRight { program, .. } => program.value(),
                ExecutedCommand::ShiftLeft { result, .. } => result.clone(),
            }
        } else {
            self.0.clone()
        }
    }

    fn status(&self) -> Vec<String> {
        let Some(ExecutedCommand::ShiftRight { name, kv, program }) = self.1.last() else {
            return vec![];
        };
        let mut status = program.status();

        let mut part = format!("{name}");
        if let Some((k, v)) = kv {
            part.push_str(&format!(" {k}: {v}"));
        }

        status.push(part);
        status
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parser::command;

    #[test]
    fn test_basic() {
        let mut interpreter = Interpreter::new("[1, 2, 3, 4]".into());
        interpreter.run(command("json").unwrap()).unwrap();
        interpreter.run(command("get 1").unwrap()).unwrap();
        assert_eq!(&*interpreter.value(), &Value::Int(2));
    }
}

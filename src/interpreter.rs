use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::data::{Function, List, SValue, Value};
use crate::parser::{Command, Expression};
use crate::{builtin, error};

struct ExecutedCommand {
    command: Command,
    result: SValue,
}

struct Context {
    functions: HashMap<String, Function>,
}

pub struct Interpreter {
    context: Arc<Context>,
    program: Vec<ExecutedCommand>,
}

impl Interpreter {
    pub fn new(input: String) -> Self {
        Self {
            context: Arc::new(Context {
                functions: builtin::builtin_functions(),
            }),
            program: vec![ExecutedCommand {
                command: Command::Expression(Expression::This),
                result: SValue::new(Value::String(input)),
            }],
        }
    }

    pub fn run(&mut self, command: Command) -> error::Result<()> {
        let this = self.value();
        match command.clone() {
            Command::Expression(e) => {
                let result = Interpreter::eval_expression(self.context.clone(), e, this)?;
                self.program.push(ExecutedCommand { command, result });
            }
            Command::ShiftLeft => todo!(),
            Command::ShiftRight => todo!(),
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        self.program.pop();
    }

    pub fn value(&self) -> SValue {
        self.program.last().unwrap().result.clone()
    }

    fn eval_expression(
        context: Arc<Context>,
        e: Expression,
        this: SValue,
    ) -> error::Result<SValue> {
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

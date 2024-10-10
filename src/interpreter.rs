use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::data::{Function, List, SValue, Value};
use crate::parser::{Command, Expression};
use crate::{builtin, error};

pub struct Interpreter {
    settings: Settings,
    program: Program,
}

struct Settings {}

struct Program {
    /// The initial value of the program
    this: SValue,
    /// The initial scope of the program
    scope: Scope,
    /// The commands that have been executed so far, that can modify scope and value
    commands: Vec<ExecutedCommand>,
}

enum ExecutedCommand {
    /// A simple expression that replaces the current value
    Expression {
        expr: Expression,

        scope: Scope,
        result: SValue,
    },
    /// An incompelte shift-right that is in progress
    ShiftRight {
        name: String,
        kv: Option<(String, String)>,

        program: Program,
    },
    /// A finished shift-right that has replaced the current value
    ShiftLeft {
        kv: Option<(Expression, Expression)>,

        scope: Scope,
        result: SValue,
    },
}

#[derive(Clone)]
pub struct Scope(Rc<HashMap<String, SValue>>);

impl Interpreter {
    pub fn new(input: String) -> Self {
        Self {
            settings: Settings {},
            program: Program {
                this: SValue::new(Value::String(input)),
                scope: Scope(Rc::new(builtin::builtin_functions())),
                commands: vec![],
            },
        }
    }

    pub fn run(&mut self, command: Command) -> error::Result<()> {
        let this = self.value();
        let mut scope = self.scope();
        match command.clone() {
            Command::Expression(e) => {
                let result = Interpreter::eval_expression(scope.clone(), e.clone(), this)?;
                self.program.commands.push(ExecutedCommand::Expression {
                    expr: e,
                    scope,
                    result,
                });
            }
            Command::ShiftRight(kv) => match (&*this, kv) {
                (Value::List(l), None) => {
                    let first = l.get(0)?.ok_or(error::Error::ShiftRightEmptySequence)?;
                    self.program.commands.push(ExecutedCommand::ShiftRight {
                        name: "list".to_string(),
                        kv: None,
                        program: Program {
                            this: first,
                            scope,
                            commands: vec![],
                        },
                    });
                }
                (Value::Dict(d), Some(kv)) => {
                    let first = d
                        .get_first()?
                        .ok_or(error::Error::ShiftRightEmptySequence)?;
                    let scope_inner = Rc::make_mut(&mut scope.0);
                    scope_inner.insert(kv.0.clone(), SValue::new(Value::String(first.0)));
                    scope_inner.insert(kv.1.clone(), first.1);
                    self.program.commands.push(ExecutedCommand::ShiftRight {
                        name: "dict".to_string(),
                        kv: Some(kv),
                        program: Program {
                            this: SValue::new(Value::Null),
                            scope,
                            commands: vec![],
                        },
                    });
                }
                (Value::Dict(_), None) => todo!(),
                _ => todo!(),
            },
            Command::ShiftLeft(kv) => todo!(),
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        self.program.commands.pop();
    }

    pub fn value(&self) -> SValue {
        self.program.value()
    }

    pub fn scope(&self) -> Scope {
        self.program.scope()
    }

    pub fn status(&self) -> Vec<String> {
        self.program.status()
    }

    fn eval_expression(scope: Scope, e: Expression, this: SValue) -> error::Result<SValue> {
        Ok(match e {
            Expression::This => this.clone(),
            Expression::Literal(l) => l,
            Expression::List(l) => SValue::new(Value::List(List {
                elements: RefCell::new(vec![]),
                rest: RefCell::new(Some(Box::new(l.into_iter().map(move |e| {
                    Interpreter::eval_expression(scope.clone(), e, this.clone())
                })))),
            })),
            Expression::Dict(_) => todo!(),
            Expression::Identifier(name) => {
                if let Some(value) = scope.0.get(&name) {
                    if let Value::Function(Function { name: name2, .. }) = value.borrow() {
                        assert_eq!(&name, name2);
                        Interpreter::eval_expression(
                            scope.clone(),
                            Expression::FunctionCall(name, vec![]),
                            this.clone(),
                        )?
                    } else {
                        value.clone()
                    }
                } else {
                    return Err(error::Error::VariableNotFound(name));
                }
            }
            Expression::FunctionCall(name, args) => {
                let Some(f) = scope.0.get(&name) else {
                    return Err(error::Error::FunctionNotFound(name));
                };
                let Value::Function(f) = f.borrow() else {
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
                    .map(|e| Interpreter::eval_expression(scope.clone(), e, this.clone()))
                    .collect::<error::Result<Vec<_>>>()?;

                (f.implementation)(args)?
            }
        })
    }
}

impl Program {
    fn value(&self) -> SValue {
        if let Some(command) = self.commands.last() {
            match command {
                ExecutedCommand::Expression { result, .. } => result.clone(),
                ExecutedCommand::ShiftRight { program, .. } => program.value(),
                ExecutedCommand::ShiftLeft { result, .. } => result.clone(),
            }
        } else {
            self.this.clone()
        }
    }

    fn scope(&self) -> Scope {
        if let Some(command) = self.commands.last() {
            match command {
                ExecutedCommand::Expression { scope, .. } => scope.clone(),
                ExecutedCommand::ShiftRight { program, .. } => program.scope(),
                ExecutedCommand::ShiftLeft { scope, .. } => scope.clone(),
            }
        } else {
            self.scope.clone()
        }
    }

    fn status(&self) -> Vec<String> {
        let Some(ExecutedCommand::ShiftRight { name, kv, program }) = self.commands.last() else {
            return vec![];
        };
        let mut status = program.status();

        let mut part = name.to_string();
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

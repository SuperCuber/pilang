use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::data::{Dict, Function, List, SValue, Value};
use crate::parser::{Command, Expression};
use crate::{builtin, error};

#[derive(Debug, Clone)]
pub struct Interpreter {
    settings: Settings,
    program: Program,
}

#[derive(Debug, Clone)]
struct Settings {}

#[derive(Debug, Clone)]
enum Program {
    Closed {
        initial: SValue,
        scope: Scope,
        commands: Vec<CachedCommand>,
    },
    Open {
        name: String,
        kv: Option<(String, String)>,
        history: Box<Program>,

        initial: SValue,
        scope: Scope,
        commands: Vec<CachedCommand>,
    },
}

#[derive(Debug, Clone)]
struct CachedCommand {
    command: ExecutedCommand,
    result: SValue,
}

#[derive(Debug, Clone)]
enum ExecutedCommand {
    Simple {
        command: Command,
    },
    Group {
        name: String,
        enter_kv: Option<(String, String)>,
        commands: Vec<ExecutedCommand>,
        leave_kv: Option<(Expression, Expression)>,
    },
}

#[derive(Debug, Clone)]
// TODO: scope should include "this", and a command can modify the scope
pub struct Scope(Rc<HashMap<String, SValue>>);

impl Interpreter {
    pub fn new(input: String) -> Self {
        Self {
            settings: Settings {},
            program: Program::Closed {
                initial: SValue::new(Value::String(input)),
                scope: Scope(Rc::new(builtin::builtin_functions())),
                commands: vec![],
            },
        }
    }

    pub fn run(&mut self, command: Command) -> error::Result<()> {
        let this = self.value();
        let mut scope = self.scope();
        match command.clone() {
            Command::Expression(expr) => {
                let result = Interpreter::eval_expression(scope.clone(), expr.clone(), this)?;
                self.program.push(CachedCommand {
                    command: ExecutedCommand::Simple { command },
                    result,
                });
            }
            Command::ShiftRight(kv) => match (&*this, kv) {
                (Value::List(l), None) => {
                    let first = l.get(0)?.ok_or(error::Error::ShiftRightEmptySequence)?;
                    replace_with::replace_with_or_abort(&mut self.program, |p| Program::Open {
                        name: "list".to_string(),
                        kv: None,
                        history: Box::new(p),

                        initial: first.clone(),
                        scope,
                        commands: vec![],
                    });
                }
                (Value::Dict(d), kv) => {
                    let kv = kv.unwrap_or(("k".into(), "v".into()));
                    let first = d
                        .get_first()?
                        .ok_or(error::Error::ShiftRightEmptySequence)?;
                    let scope_inner = Rc::make_mut(&mut scope.0);
                    scope_inner.insert(kv.0.clone(), SValue::new(Value::String(first.0)));
                    scope_inner.insert(kv.1.clone(), first.1);
                    replace_with::replace_with_or_abort(&mut self.program, |p| Program::Open {
                        name: "dict".to_string(),
                        kv: Some(kv),
                        history: Box::new(p),

                        initial: SValue::new(Value::Null),
                        scope,
                        commands: vec![],
                    });
                }
                _ => todo!("invalid shift right"),
            },
            Command::ShiftLeft(leave_kv) => {
                let Program::Open {
                    name,
                    kv: enter_kv,
                    mut history,
                    initial,
                    scope,
                    commands,
                } = self.program.clone()
                else {
                    return Err(error::Error::ShiftLeftNotInShift);
                };
                // "this" before that was the preview of the first element,
                // now we care about the whole container
                let this = history.value();
                let mut iterable: Box<dyn Iterator<Item = _>> = match &*this {
                    Value::List(l) => Box::new(List::into_iter(this.clone())),
                    Value::Dict(d) => Box::new(Dict::into_iter(this.clone()).map(|r| {
                        r.map(|(k, v)| {
                            SValue::new(Value::List(List {
                                elements: vec![SValue::new(Value::String(k)), v].into(),
                                rest: None.into(),
                            }))
                        })
                    })),
                    _ => unreachable!("shifting left when last value is non sequence"),
                };

                if let Some((k_var, v_var)) = enter_kv {
                    todo!()
                } else {
                    let commands = commands.clone();
                    let interpreter = self.clone();
                    let history = history.clone();
                    iterable = Box::new(iterable.map(move |e| -> error::Result<_> {
                        let e = e?;
                        let mut interpreter = Interpreter {
                            settings: interpreter.settings.clone(),
                            program: Program::Closed {
                                initial: e,
                                scope: scope.clone(),
                                commands: vec![],
                            },
                        };
                        for command in &commands {
                            interpreter.rerun(&command.command)?;
                        }
                        Ok(interpreter.value())
                    }));
                }
                let result = if let Some((k_var, v_var)) = leave_kv {
                    todo!()
                } else {
                    SValue::new(Value::List(List {
                        elements: RefCell::new(vec![]),
                        rest: RefCell::new(Some(iterable)),
                    }))
                };
                history.push(CachedCommand {
                    command: ExecutedCommand::Group {
                        name,
                        enter_kv,
                        commands: commands.clone().into_iter().map(|c| c.command).collect(),
                        leave_kv,
                    },
                    result,
                });
                replace_with::replace_with_or_abort(&mut self.program, |p| *history);
            }
        }
        Ok(())
    }

    fn rerun(&mut self, command: &ExecutedCommand) -> error::Result<()> {
        match command {
            ExecutedCommand::Simple { command } => self.run(command.clone()),
            ExecutedCommand::Group {
                name,
                enter_kv,
                commands,
                leave_kv,
            } => {
                self.run(Command::ShiftRight(enter_kv.clone()))?;
                for command in commands {
                    self.rerun(command)?;
                }
                self.run(Command::ShiftLeft(leave_kv.clone()))
            }
        }
    }

    pub fn undo(&mut self) {
        self.program.pop();
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

            Expression::Plus(x, y) => {
                match eval_number_pair(this.clone(), scope.clone(), *x.clone(), *y.clone()) {
                    Ok((x, y)) => SValue::new(Value::Float(x + y)),
                    Err(_) => {
                        let x = Interpreter::eval_expression(scope.clone(), *x, this.clone())?;
                        let x = x
                            .as_string()
                            .ok_or(error::Error::InvalidTypes(&["string", "number"]))?;
                        let y = Interpreter::eval_expression(scope.clone(), *y, this.clone())?;
                        let y = y
                            .as_string()
                            .ok_or(error::Error::InvalidTypes(&["string", "number"]))?;
                        SValue::new(Value::String(format!("{}{}", x, y)))
                    }
                }
            }
            Expression::Minus(x, y) => {
                let (x, y) = eval_number_pair(this.clone(), scope.clone(), *x, *y)?;
                SValue::new(Value::Float(x - y))
            }
            Expression::UnaryMinus(x) => {
                let x = Interpreter::eval_expression(scope.clone(), *x, this.clone())?
                    .as_number()
                    .ok_or(error::Error::InvalidType("number"))?;
                SValue::new(Value::Float(-x))
            }
            Expression::Multiply(x, y) => {
                let (x, y) = eval_number_pair(this.clone(), scope.clone(), *x, *y)?;
                SValue::new(Value::Float(x * y))
            }
            Expression::Divide(x, y) => {
                let (x, y) = eval_number_pair(this.clone(), scope.clone(), *x, *y)?;
                SValue::new(Value::Float(x / y))
            }
            Expression::And(x, y) => {
                let x = Interpreter::eval_expression(scope.clone(), *x, this.clone())?
                    .as_bool()
                    .ok_or(error::Error::InvalidType("boolean"))?;
                if x {
                    Interpreter::eval_expression(scope.clone(), *y, this.clone())?
                } else {
                    SValue::new(Value::Bool(false))
                }
            }
            Expression::Or(x, y) => {
                let x = Interpreter::eval_expression(scope.clone(), *x, this.clone())?
                    .as_bool()
                    .ok_or(error::Error::InvalidType("boolean"))?;
                if x {
                    SValue::new(Value::Bool(true))
                } else {
                    Interpreter::eval_expression(scope.clone(), *y, this.clone())?
                }
            }

            Expression::List(l) => SValue::new(Value::List(List {
                elements: RefCell::new(
                    l.into_iter()
                        .map(move |e| Interpreter::eval_expression(scope.clone(), e, this.clone()))
                        .collect::<Result<_, _>>()?,
                ),
                rest: RefCell::new(None),
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

fn eval_number_pair(
    this: SValue,
    scope: Scope,
    x: Expression,
    y: Expression,
) -> error::Result<(f64, f64)> {
    Ok((
        Interpreter::eval_expression(scope.clone(), x, this.clone())?
            .as_number()
            .ok_or(error::Error::InvalidType("number"))?,
        Interpreter::eval_expression(scope.clone(), y, this.clone())?
            .as_number()
            .ok_or(error::Error::InvalidType("number"))?,
    ))
}

impl Program {
    fn value(&self) -> SValue {
        let (initial, commands) = match self {
            Program::Closed {
                initial, commands, ..
            } => (initial, commands),
            Program::Open {
                initial, commands, ..
            } => (initial, commands),
        };

        if let Some(commands) = commands.last() {
            commands.result.clone()
        } else {
            initial.clone()
        }
    }

    fn scope(&self) -> Scope {
        match self {
            Program::Closed { scope, .. } => scope,
            Program::Open { scope, .. } => scope,
        }
        .clone()
    }

    pub fn push(&mut self, command: CachedCommand) {
        let commands = match self {
            Program::Closed { commands, .. } => commands,
            Program::Open { commands, .. } => commands,
        };
        commands.push(command);
    }

    pub fn pop(&mut self) {
        let commands = match self {
            Program::Closed { commands, .. } => commands,
            Program::Open { commands, .. } => commands,
        };
        // TODO: undo just the shift-left by replacing self with the
        // `commands` and history and stuff
        commands.pop();
    }

    fn status(&self) -> Vec<String> {
        let mut result = vec![];
        let mut program = self;
        loop {
            match program {
                Program::Closed { .. } => break,
                Program::Open {
                    name, kv, history, ..
                } => {
                    let kv = kv.as_ref().map(|(k, v)| format!("{}: {}", k, v));
                    result.push(format!("{} ({})", name, kv.unwrap_or_default()));
                    program = history;
                }
            }
        }
        result.reverse();
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parser::command;

    #[test]
    fn test_shifting() {
        let mut interpreter = Interpreter::new("[1, 2, 3, 4]".into());
        interpreter.run(command("json").unwrap()).unwrap();
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(&*interpreter.value(), &Value::Int(1));
        interpreter.run(command("<<").unwrap()).unwrap();
        interpreter.value().sample().unwrap();
        assert_eq!(
            &*interpreter.value(),
            &Value::List(List {
                elements: vec![
                    SValue::new(Value::Int(1)),
                    SValue::new(Value::Int(2)),
                    SValue::new(Value::Int(3)),
                    // lazy
                ]
                .into(),
                rest: None.into(),
            })
        );
    }

    #[test]
    fn test_nesting() {
        let mut interpreter = Interpreter::new("".into());
        interpreter
            .run(command("[[1,2,3],[4,5,6],[7,8,9]]").unwrap())
            .unwrap();
        interpreter.run(command(">>").unwrap()).unwrap();
        assert!(interpreter.value().as_list().is_some());
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(&*interpreter.value(), &Value::Int(1));
        interpreter.run(command("100").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        interpreter.value().sample().unwrap();
        panic!("{:?}", interpreter.value());
    }
}

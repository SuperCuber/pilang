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
    use crate::data::{List, SValue, Value};
    use crate::parser::command;

    fn init_interpreter(json_str: &str) -> Interpreter {
        let mut interpreter = Interpreter::new(json_str.into());
        interpreter.run(command("json").unwrap()).unwrap();
        interpreter
    }

    fn s_list(elements: Vec<SValue>) -> SValue {
        SValue::new(Value::List(List {
            elements: RefCell::new(elements),
            rest: RefCell::new(None),
        }))
    }

    fn s_int(val: u64) -> SValue {
        SValue::new(Value::Int(val))
    }

    fn s_float(val: f64) -> SValue {
        SValue::new(Value::Float(val))
    }

    fn s_string(val: &str) -> SValue {
        SValue::new(Value::String(val.to_string()))
    }

    fn s_bool(val: bool) -> SValue {
        SValue::new(Value::Bool(val))
    }

    fn s_null() -> SValue {
        SValue::new(Value::Null)
    }

    #[test]
    fn test_json_parsing_and_basic_literals() {
        let interpreter = init_interpreter("[1, \"test\", true, null, 2.5]");
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_int(1)));
        assert_eq!(l.get(1).unwrap(), Some(s_string("test")));
        assert_eq!(l.get(2).unwrap(), Some(s_bool(true)));
        assert_eq!(l.get(3).unwrap(), Some(s_null()));
        assert_eq!(l.get(4).unwrap(), Some(s_float(2.5)));

        let mut interpreter_invalid = Interpreter::new("[1, 2".into());
        let res = interpreter_invalid.run(command("json").unwrap());
        assert!(matches!(res, Err(error::Error::BuiltinFunctionError(_))));
    }

    #[test]
    fn test_arithmetic_expressions() {
        let mut interpreter = init_interpreter("0");
        interpreter.run(command("1 + 2").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(3.0));
        interpreter.run(command("% - 1").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(2.0));
        interpreter.run(command("10 * %").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(20.0));
        interpreter.run(command("% / 4").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(5.0));
        interpreter.run(command("-%").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(-5.0));
        interpreter = init_interpreter("\"hello\"");
        interpreter.run(command("% + \" world\"").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_string("hello world"));
    }

    #[test]
    fn test_builtin_get() {
        let mut interpreter = init_interpreter("{\"key\": \"value\", \"arr\": [10, 20]}");
        interpreter.run(command("get \"key\"").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_string("value"));

        interpreter = init_interpreter("{\"key\": \"value\", \"arr\": [10, 20]}");
        interpreter.run(command("get \"arr\"").unwrap()).unwrap();
        interpreter.run(command("get 1").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(20));

        interpreter = init_interpreter("{}");
        interpreter
            .run(command("get \"nonexistent\"").unwrap())
            .unwrap();
        assert_eq!(interpreter.value(), s_null());

        interpreter = init_interpreter("[1]");
        let res = interpreter.run(command("get 1").unwrap());
        assert!(matches!(res, Err(error::Error::BuiltinFunctionError(_))));
    }

    #[test]
    fn test_builtin_assoc() {
        let mut interpreter = init_interpreter("{\"a\": 1}");
        interpreter.run(command("assoc \"b\" 2").unwrap()).unwrap();
        let val = interpreter.value();
        let d = val.as_dict().unwrap();
        assert_eq!(d.get("a").unwrap(), Some(s_int(1)));
        assert_eq!(d.get("b").unwrap(), Some(s_int(2)));

        interpreter = init_interpreter("[1,2,3]");
        interpreter.run(command("assoc 1 100").unwrap()).unwrap();
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_int(1)));
        assert_eq!(l.get(1).unwrap(), Some(s_int(100)));
        assert_eq!(l.get(2).unwrap(), Some(s_int(3)));

        interpreter = init_interpreter("[1]");
        let res = interpreter.run(command("assoc 1 100").unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn test_shift_list_simple_transform() {
        let mut interpreter = init_interpreter("[1,2,3]");
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(interpreter.status(), vec!["list ()".to_string()]);
        assert_eq!(interpreter.value(), s_int(1));
        interpreter.run(command("% + 10").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(11.0));
        interpreter.run(command("<<").unwrap()).unwrap();
        assert_eq!(interpreter.status(), Vec::<String>::new());

        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_float(11.0)));
        assert_eq!(l.get(1).unwrap(), Some(s_float(12.0)));
        assert_eq!(l.get(2).unwrap(), Some(s_float(13.0)));
    }

    #[test]
    fn test_shift_list_nested() {
        let mut interpreter = init_interpreter("[[1,2,3],[4,5,6],[7,8,9]]");
        interpreter.run(command(">>").unwrap()).unwrap();
        assert!(interpreter.value().as_list().is_some());
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(1));
        interpreter.run(command("100").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();

        let val = interpreter.value();
        let outer_l = val.as_list().unwrap();
        for i in 0..3 {
            let outer_elem = outer_l.get(i).unwrap().unwrap();
            let inner_l = outer_elem.as_list().unwrap();
            for j in 0..3 {
                assert_eq!(inner_l.get(j).unwrap(), Some(s_int(100)));
            }
        }
    }

    #[test]
    fn test_shift_empty_list() {
        let mut interpreter = init_interpreter("[]");
        let res_shift_right = interpreter.run(command(">>").unwrap());
        assert!(matches!(
            res_shift_right,
            Err(error::Error::ShiftRightEmptySequence)
        ));
        let val = interpreter.value();
        let list = val.as_list().unwrap();
        assert!(list.get(0).unwrap().is_none());

        let mut interpreter2 = init_interpreter("[]");
        let res_shift_left = interpreter2.run(command("<<").unwrap());
        assert!(matches!(
            res_shift_left,
            Err(error::Error::ShiftLeftNotInShift)
        ));
    }

    #[test]
    #[ignore = "The dict shift with k:v is not implemented yet"]
    fn test_shift_dict_collect_values() {
        let mut interpreter = init_interpreter("{\"a\": 1, \"b\": 2}");
        interpreter.run(command(">> k:v").unwrap()).unwrap();
        assert_eq!(interpreter.status(), vec!["dict (k: v)".to_string()]);
        interpreter.run(command("v").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(1));

        interpreter.run(command("<<").unwrap()).unwrap();
        assert_eq!(interpreter.status(), Vec::<String>::new());
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_int(1)));
        assert_eq!(l.get(1).unwrap(), Some(s_int(2)));
    }

    #[test]
    #[ignore = "The k command in dict shift is not properly collecting keys into a list"]
    fn test_shift_dict_collect_keys() {
        let mut interpreter = init_interpreter("{\"a\": 1, \"b\": 2}");
        interpreter.run(command(">> k:v").unwrap()).unwrap();
        assert_eq!(interpreter.status(), vec!["dict (k: v)".to_string()]);
        interpreter.run(command("k").unwrap()).unwrap();

        assert_eq!(interpreter.status(), vec!["dict (k: v)".to_string()]);
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_string("a")));
        assert_eq!(l.get(1).unwrap(), Some(s_string("b")));
    }

    #[test]
    fn test_undo_simple_command() {
        let mut interpreter = init_interpreter("10");
        interpreter.run(command("% + 5").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(15.0));
        interpreter.undo();
        assert_eq!(interpreter.value(), s_int(10));
    }

    #[test]
    fn test_undo_multiple_commands() {
        let mut interpreter = init_interpreter("0");
        interpreter.run(command("10").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(10));
        interpreter.run(command("% + 5").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(15.0));
        interpreter.undo();
        assert_eq!(interpreter.value(), s_int(10));
        interpreter.undo();
        assert_eq!(interpreter.value(), s_int(0));
    }

    #[test]
    fn test_undo_after_shift_block() {
        let mut interpreter = init_interpreter("[1,2]");
        interpreter.run(command(">>").unwrap()).unwrap();
        interpreter.run(command("% + 1").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_float(2.0)));
        assert_eq!(l.get(1).unwrap(), Some(s_float(3.0)));

        interpreter.undo();
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_int(1)));
        assert_eq!(l.get(1).unwrap(), Some(s_int(2)));
    }

    #[test]
    fn test_undo_in_open_state() {
        let mut interpreter = init_interpreter("[1,2,3]");
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(interpreter.status(), vec!["list ()".to_string()]);
        assert_eq!(interpreter.value(), s_int(1));
        interpreter.run(command("% + 10").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_float(11.0));
        interpreter.undo();
        assert_eq!(interpreter.value(), s_int(1));
        assert_eq!(interpreter.status(), vec!["list ()".to_string()]);
        interpreter.run(command("<<").unwrap()).unwrap();

        let val = interpreter.value();
        let l = val.as_list().unwrap();
        assert_eq!(l.get(0).unwrap(), Some(s_int(1)));
    }

    #[test]
    fn test_error_variable_not_found() {
        let mut interpreter = init_interpreter("{}");
        let res = interpreter.run(command("x").unwrap());
        assert!(matches!(res, Err(error::Error::VariableNotFound(_))));
    }

    #[test]
    fn test_error_invalid_type_in_expression() {
        let mut interpreter = init_interpreter("\"text\"");
        let res = interpreter.run(command("% * 2").unwrap());
        assert!(matches!(res, Err(error::Error::InvalidType("number"))));
    }

    #[test]
    fn test_function_arity_error() {
        let mut interpreter = init_interpreter("0");
        let res = interpreter.run(command("get").unwrap());
        assert!(
            matches!(res, Err(error::Error::InvalidArity(s,0,v)) if s == "get" && v == vec![2])
        );

        let res_many = interpreter.run(command("get 1 2 3").unwrap());
        assert!(
            matches!(res_many, Err(error::Error::InvalidArity(s,3,v)) if s == "get" && v == vec![2])
        );
    }

    #[test]
    fn test_shifting() {
        let mut interpreter = Interpreter::new("[1, 2, 3, 4]".into());
        interpreter.run(command("json").unwrap()).unwrap();
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(1));
        interpreter.run(command("<<").unwrap()).unwrap();

        if let Value::List(l) = &*interpreter.value() {
            assert_eq!(l.get(0).unwrap().unwrap(), s_int(1));
            assert_eq!(l.get(1).unwrap().unwrap(), s_int(2));
            assert_eq!(l.get(2).unwrap().unwrap(), s_int(3));
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_nesting() {
        let mut interpreter = Interpreter::new("".into());
        interpreter
            .run(command("json \"[[1,2,3],[4,5,6],[7,8,9]]\"").unwrap())
            .unwrap();
        interpreter.run(command(">>").unwrap()).unwrap();
        assert!(interpreter.value().as_list().is_some());
        interpreter.run(command(">>").unwrap()).unwrap();
        assert_eq!(interpreter.value(), s_int(1));
        interpreter.run(command("100").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        interpreter.run(command("<<").unwrap()).unwrap();
        let val = interpreter.value();
        let l = val.as_list().unwrap();
        for i in 0..3 {
            let outer_elem = l.get(i).unwrap().unwrap();
            let inner_l = outer_elem.as_list().unwrap();
            for j in 0..3 {
                assert_eq!(inner_l.get(j).unwrap(), Some(s_int(100)));
            }
        }
    }
}

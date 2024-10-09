use std::cell::RefCell;
use std::collections::HashMap;

use crate::data::{Function, List, SValue, Value};
use crate::parser::{Command, Expression};

struct ExecutedCommand {
    command: Command,
    result: SValue,
}

pub struct Interpreter {
    functions: HashMap<String, Function>,
    program: Vec<ExecutedCommand>,
}

impl Interpreter {
    pub fn new(input: String) -> Self {
        Self {
            functions: HashMap::new(),
            program: vec![ExecutedCommand {
                command: Command::Expression(Expression::This),
                result: SValue::new(Value::String(input)),
            }],
        }
    }

    pub fn run(&mut self, command: Command) {
        let this = self.value();
        match command {
            Command::Expression(e) => {
                let result = self.eval_expression(e, this);
            }
            Command::ShiftLeft => todo!(),
            Command::ShiftRight => todo!(),
        }
    }

    pub fn show_sample(&self) {
        todo!()
    }

    fn value(&self) -> SValue {
        self.program.last().unwrap().result.clone()
    }

    fn eval_expression(&self, e: Expression, this: SValue) -> SValue {
        match e {
            Expression::This => this.clone(),
            Expression::Literal(l) => SValue::new(l),
            Expression::List(l) => {
                let evaluated = l
                    .into_iter()
                    .map(move |e| self.eval_expression(e, this.clone()));
                SValue::new(Value::List(List {
                    elements: RefCell::new(evaluated.collect()),
                    rest: RefCell::new(None),
                }))
            }
            Expression::Dict(_) => todo!(),
            Expression::FunctionCall(_, _) => todo!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic() {
        let interpreter = Interpreter::new("[1, 2, 3, 4]".into());
        // interpreter.run(Command::Expression(Expression::Literal(Value::Number(123))));
        assert_eq!(&*interpreter.value(), &Value::Number(123));
    }
}

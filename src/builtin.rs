use indexmap::IndexMap;

use std::collections::HashMap;

use crate::{
    data::{Function, SValue, Value},
    error,
};

pub fn builtin_functions() -> HashMap<String, SValue> {
    let mut functions = HashMap::new();
    functions.insert(
        "json".to_string(),
        Function {
            name: "json".to_string(),
            arities: vec![1],
            implementation: Box::new(json),
        },
    );
    functions.insert(
        "get".to_string(),
        Function {
            name: "get".to_string(),
            arities: vec![2],
            implementation: Box::new(get),
        },
    );
    functions.insert(
        "assoc".to_string(),
        Function {
            name: "assoc".to_string(),
            arities: vec![3],
            implementation: Box::new(assoc),
        },
    );

    functions
        .into_iter()
        .map(|(k, v)| (k, SValue::new(Value::Function(v))))
        .collect()
}

fn get(mut args: Vec<SValue>) -> error::Result<SValue> {
    assert!(
        args.len() == 2,
        "get function expects exactly two arguments"
    );
    let key = args.remove(1);
    let container = args.remove(0);

    match &*key {
        Value::String(s) => {
            let Value::Dict(dict) = &*container else {
                return Err(error::Error::BuiltinFunctionError(format!(
                    "get function expects a dict as the first argument, got {:?}",
                    container
                )));
            };
            let value = dict
                .elements
                .borrow()
                .get(s)
                .cloned()
                .unwrap_or_else(|| SValue::new(Value::Null));
            Ok(value)
        }
        Value::Int(n) => {
            let Value::List(list) = &*container else {
                return Err(error::Error::BuiltinFunctionError(format!(
                    "get function expects a list as the first argument, got {:?}",
                    container
                )));
            };
            list.get(*n as usize)?.ok_or_else(|| {
                error::Error::BuiltinFunctionError(format!("index out of bounds: {}", n))
            })
        }
        _ => Err(error::Error::BuiltinFunctionError(
            "get function expects a string or an integer as the second argument".to_string(),
        )),
    }
}

fn assoc(mut args: Vec<SValue>) -> error::Result<SValue> {
    assert!(
        args.len() == 3,
        "assoc function expects exactly three arguments"
    );
    let value = args.remove(2);
    let key = args.remove(1);
    let container = args.remove(0);

    match &*key {
        Value::String(s) => {
            let Value::Dict(dict) = &*container else {
                return Err(error::Error::BuiltinFunctionError(format!(
                    "assoc function expects a dict as the first argument, got {container}",
                )));
            };
            dict.realize_all()?;
            // lazy_rest is None, so we can just copy the elements
            let mut elements = dict.elements.borrow().clone();
            elements.insert(s.clone(), value);
            Ok(SValue::new(Value::Dict(crate::data::Dict {
                elements: elements.into(),
                rest: None.into(),
            })))
        }
        Value::Int(n) => {
            let Value::List(list) = &*container else {
                return Err(error::Error::BuiltinFunctionError(format!(
                    "assoc function expects a list as the first argument, got {container}",
                )));
            };
            list.realize_all()?;
            let mut elements = list.elements.borrow().clone();
            if let Some(e) = elements.get_mut(*n as usize) {
                *e = value;
            } else {
                return Err(error::Error::BuiltinFunctionError(format!(
                    "index out of bounds: {n}",
                )));
            }
            Ok(SValue::new(Value::List(crate::data::List {
                elements: elements.into(),
                rest: None.into(),
            })))
        }
        _ => Err(error::Error::BuiltinFunctionError(
            "assoc function expects a string or an integer as the second argument".to_string(),
        )),
    }
}

fn json(mut args: Vec<SValue>) -> error::Result<SValue> {
    assert!(
        args.len() == 1,
        "json function expects exactly one argument"
    );
    let arg = args.remove(0);
    let Value::String(s) = &*arg else {
        return Err(error::Error::BuiltinFunctionError(format!(
            "json function expects a string, got {:?}",
            arg
        )));
    };

    let parsed: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| error::Error::BuiltinFunctionError(format!("failed to parse JSON: {}", e)))?;

    Ok(SValue::new(Value::from(parsed)))
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(n) = n.as_u64() {
                    Value::Int(n)
                } else if let Some(n) = n.as_f64() {
                    Value::Float(n)
                } else {
                    panic!("failed to convert JSON number {:?} to u32 or f32", n)
                }
            }
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(a) => {
                let vals: Vec<_> = a
                    .into_iter()
                    .map(|v| SValue::new(Value::from(v)))
                    .collect::<Vec<_>>();
                Value::List(crate::data::List {
                    elements: vals.into(),
                    rest: None.into(),
                })
            }
            serde_json::Value::Object(o) => {
                let vals: IndexMap<_, _> = o
                    .into_iter()
                    .map(|(k, v)| (k, SValue::new(Value::from(v))))
                    .collect();
                Value::Dict(crate::data::Dict {
                    elements: vals.into(),
                    rest: None.into(),
                })
            }
        }
    }
}

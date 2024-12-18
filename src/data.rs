use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

use crate::error;

/// Shared value
pub type SValue = Rc<Value>;

#[derive(Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(u64),
    Float(f64),
    // TODO: strings can be lazy?
    String(String),
    List(List),
    Dict(Dict),
    Function(Function),
}

type LazyRest<T> = RefCell<Option<Box<dyn Iterator<Item = error::Result<T>>>>>;

/// Lazily evaluated list
pub struct List {
    pub elements: RefCell<Vec<SValue>>,
    pub rest: LazyRest<SValue>,
}

/// Lazily evaluated dict
pub struct Dict {
    pub elements: RefCell<IndexMap<String, SValue>>,
    pub rest: LazyRest<(String, SValue)>,
}

pub struct Function {
    pub name: String,
    pub arities: Vec<usize>,
    pub implementation: Box<dyn Fn(Vec<SValue>) -> error::Result<SValue>>,
}

// Impls

impl Value {
    /// Realize the inner value enough for a user to have a good ol' look at it
    pub fn sample(&self) -> error::Result<()> {
        // TODO: replace simple "3" heuristic with something better. maybe recursive "size/complexity estimation"
        match self {
            Value::List(l) => {
                l.realize_n(3)?;
                for e in l.elements.borrow().iter() {
                    e.sample()?;
                }
            }
            Value::Dict(m) => {
                m.realize_n(3)?;
                for e in m.elements.borrow().values() {
                    e.sample()?;
                }
            }
            _ => (),
        }
        Ok(())
    }

    pub fn realize(&self) -> error::Result<()> {
        match self {
            Value::List(l) => l.realize_all()?,
            Value::Dict(m) => m.realize_all()?,
            _ => (),
        }
        Ok(())
    }

    pub(crate) fn as_dict(&self) -> Option<&Dict> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn as_list(&self) -> Option<&List> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    pub(crate) fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl List {
    pub fn get(&self, n: usize) -> error::Result<Option<SValue>> {
        self.realize_n(n + 1)?;
        Ok(self.elements.borrow().get(n).cloned())
    }

    pub fn realize_all(&self) -> error::Result<()> {
        if let Some(rest) = self.rest.take() {
            let mut elems = self.elements.borrow_mut();
            for elem in rest {
                elems.push(elem?);
            }
        }
        Ok(())
    }

    /// Expand to length n
    fn realize_n(&self, n: usize) -> error::Result<()> {
        let mut elements_needed = n.saturating_sub(self.elements.borrow().len());

        if let Some(rest) = self.rest.borrow_mut().as_mut() {
            while elements_needed > 0 {
                let mut elems = self.elements.borrow_mut();
                if let Some(next) = rest.next() {
                    let next = next?;
                    elems.push(next);
                    elements_needed -= 1;
                } else {
                    break;
                }
            }
        }
        if elements_needed > 0 {
            *self.rest.borrow_mut() = None;
        }

        Ok(())
    }

    pub fn into_iter(this: SValue) -> ListIter {
        ListIter {
            list: this,
            index: 0,
        }
    }
}

pub struct ListIter {
    list: SValue,
    index: usize,
}

impl Iterator for ListIter {
    type Item = error::Result<SValue>;

    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1;
        self.list.as_list().unwrap().get(self.index - 1).transpose()
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s), // TODO: hide the rest if its too much
            Value::List(l) => write!(f, "{}", l),
            Value::Dict(m) => write!(f, "{}", m),
            Value::Function(func) => write!(f, "<builtin function {}>", func.name),
        }
    }
}

impl std::fmt::Debug for List {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("List")
            .field("elements", &self.elements)
            .field("lazy_extra", &self.rest.borrow().is_some())
            .finish()
    }
}

// TODO: hide the rest if its too much
impl std::fmt::Display for List {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        let elements = self.elements.borrow_mut();
        let mut iter = elements.iter();
        if let Some(first) = iter.next() {
            write!(f, "{}", first)?;
            for elem in iter {
                write!(f, ", {}", elem)?;
            }
            if self.rest.borrow().is_some() {
                write!(f, ", ...")?;
            }
        } else if self.rest.borrow().is_some() {
            write!(f, "...")?;
        }
        write!(f, "]")
    }
}

impl std::cmp::PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

impl Dict {
    pub fn get(&self, key: &str) -> error::Result<Option<SValue>> {
        self.realize_look_for(key)?;
        Ok(self.elements.borrow().get(key).cloned())
    }

    fn get_nth(&self, n: usize) -> error::Result<Option<(String, SValue)>> {
        self.realize_n(n + 1)?;
        self.elements
            .borrow()
            // IndexMap
            .get_index(n)
            .map(|(k, v)| Ok((k.clone(), v.clone())))
            .transpose()
    }

    pub fn get_first(&self) -> error::Result<Option<(String, SValue)>> {
        self.realize_n(1)?;
        Ok(self
            .elements
            .borrow()
            .iter()
            .next()
            .map(|(k, v)| (k.clone(), v.clone())))
    }

    /// Expand to size n
    pub fn realize_n(&self, n: usize) -> error::Result<()> {
        let mut elements_needed = (n + 1).saturating_sub(self.elements.borrow().len());

        if let Some(rest) = self.rest.borrow_mut().as_mut() {
            while elements_needed > 0 {
                let mut elems = self.elements.borrow_mut();
                if let Some(next) = rest.next() {
                    let (k, v) = next?;
                    elems.insert(k, v);
                    elements_needed -= 1;
                } else {
                    break;
                }
            }
        }
        if elements_needed > 0 {
            *self.rest.borrow_mut() = None;
        }

        Ok(())
    }

    pub fn realize_look_for(&self, key: &str) -> error::Result<Option<SValue>> {
        if let Some(rest) = self.rest.take() {
            let mut elems = self.elements.borrow_mut();
            for elem in rest {
                let (k, v) = elem?;
                elems.insert(k.clone(), v.clone());
                if k == key {
                    return Ok(Some(v));
                }
            }
        }
        Ok(self.elements.borrow().get(key).cloned())
    }

    pub fn realize_all(&self) -> error::Result<()> {
        if let Some(rest) = self.rest.take() {
            let mut elems = self.elements.borrow_mut();
            for elem in rest {
                let (k, v) = elem?;
                elems.insert(k, v);
            }
        }
        Ok(())
    }

    pub fn into_iter(this: SValue) -> DictIter {
        DictIter {
            dict: this,
            index: 0,
        }
    }
}

impl std::fmt::Debug for Dict {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Dict")
            .field("elements", &self.elements)
            .field("lazy_extra", &self.rest.borrow().is_some())
            .finish()
    }
}

// TODO: hide the rest if its too much
impl std::fmt::Display for Dict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let elements = self.elements.borrow_mut();
        let mut iter = elements.iter();
        if let Some((k, v)) = iter.next() {
            write!(f, "{}: {}", k, v)?;
            for (k, v) in iter {
                write!(f, ", {}: {}", k, v)?;
            }
            if self.rest.borrow().is_some() {
                write!(f, ", ...")?;
            }
        } else if self.rest.borrow().is_some() {
            write!(f, "...")?;
        }
        write!(f, "}}")
    }
}

impl std::cmp::PartialEq for Dict {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

pub struct DictIter {
    dict: SValue,
    index: usize,
}

impl Iterator for DictIter {
    type Item = error::Result<(String, SValue)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1;
        self.dict
            .as_dict()
            .unwrap()
            .get_nth(self.index - 1)
            .transpose()
    }
}

impl std::fmt::Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Function")
            .field("name", &self.name)
            .field("arities", &self.arities)
            .finish()
    }
}

impl std::cmp::PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

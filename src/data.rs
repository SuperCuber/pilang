use std::{borrow::BorrowMut, cell::RefCell, collections::HashMap, ops::Deref, sync::Arc};

use crate::error;

/// Shared value
pub type SValue = Arc<Value>;

#[derive(Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(u64),
    Float(f64),
    // TODO: strings can be lazy?
    String(String),
    List(List),
    Map(Dict),
}

type LazyRest<T> = RefCell<Option<Box<dyn Iterator<Item = error::Result<T>>>>>;

/// Lazily evaluated list
pub struct List {
    pub elements: RefCell<Vec<SValue>>,
    pub rest: LazyRest<SValue>,
}

/// Lazily evaluated map
pub struct Dict {
    pub elements: RefCell<HashMap<String, SValue>>,
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
            Value::List(l) => l.realize_n(3),
            Value::Map(m) => m.realize_n(3),
            _ => Ok(()),
        }
    }
}

impl List {
    pub fn get(&self, n: usize) -> error::Result<Option<SValue>> {
        self.realize_n(n + 1)?;
        Ok(self.elements.borrow().get(n).cloned())
    }

    pub fn realize_all(&self) -> error::Result<()> {
        if let Some(mut rest) = self.rest.take() {
            let mut elems = self.elements.borrow_mut();
            while let Some(next) = rest.next() {
                let next = next?;
                elems.push(next);
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
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::List(l) => {
                write!(f, "{}", l)
            }
            Value::Map(m) => {
                write!(f, "{}", m)
            }
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
        } else {
            if self.rest.borrow().is_some() {
                write!(f, "...")?;
            }
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
        self.realize_all()?;
        Ok(self.elements.borrow().get(key).cloned())
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

    pub fn realize_all(&self) -> error::Result<()> {
        if let Some(mut rest) = self.rest.take() {
            let mut elems = self.elements.borrow_mut();
            while let Some(next) = rest.next() {
                let (k, v) = next?;
                elems.insert(k, v);
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Dict {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Map")
            .field("elements", &self.elements)
            .field("lazy_extra", &self.rest.borrow().is_some())
            .finish()
    }
}

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
        } else {
            if self.rest.borrow().is_some() {
                write!(f, "...")?;
            }
        }
        write!(f, "}}")
    }
}

impl std::cmp::PartialEq for Dict {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

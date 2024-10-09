use std::{cell::RefCell, collections::HashMap, ops::Deref, sync::Arc};

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

impl List {
    pub fn get(& self, n: usize) -> error::Result<Option<SValue>> {
        self.realize(n)?;
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

    fn realize(&self, n: usize) -> error::Result<()> {
        let elements_needed = (n + 1).saturating_sub(self.elements.borrow().len());
        if elements_needed > 0 {
            if let Some(rest) = self.rest.borrow_mut().as_mut() {
                let mut rest = rest.take(elements_needed);
                let mut elems = self.elements.borrow_mut();
                while let Some(next) = rest.next() {
                    let next = next?;
                    elems.push(next);
                }
            }
        }
        Ok(())
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

impl std::cmp::PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

impl Dict {
    pub fn get(& mut self, key: &str) -> error::Result<Option<SValue>> {
        self.realize_all()?;
        Ok(self.elements.borrow().get(key).cloned())
    }

    pub fn realize_all(&mut self) -> error::Result<()> {
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

impl std::cmp::PartialEq for Dict {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

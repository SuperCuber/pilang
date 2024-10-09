use std::{cell::RefCell, collections::HashMap, ops::Deref, sync::Arc};

/// Shared value
pub type SValue = Arc<Value>;

#[derive(Debug, PartialEq)]
pub enum Value {
    Number(u32),
    // TODO: strings can be lazy?
    String(String),
    List(List),
    Map(Dict),
}

type LazyRest<T> = RefCell<Option<Box<dyn Iterator<Item = T>>>>;

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
    pub implementation: Box<dyn Fn(Vec<SValue>) -> SValue>,
}

impl List {
    pub fn get(&self, n: usize) -> Option<SValue> {
        self.realize(n);
        self.elements.borrow().get(n).cloned()
    }

    pub fn realize_all(&self) {
        if let Some(rest) = self.rest.take() {
            self.elements.borrow_mut().extend(rest);
        }
    }

    fn realize(&self, n: usize) {
        let elements_needed = n - self.elements.borrow().len();
        if elements_needed > 0 {
            if let Some(rest) = self.rest.borrow_mut().as_mut() {
                // Safety: a list will not borrow itself while realizing
                self.elements
                    .borrow_mut()
                    .extend(rest.take(elements_needed));
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

impl std::cmp::PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}

impl Dict {
    pub fn get(&mut self, key: &str) -> Option<SValue> {
        self.realize_all();
        self.elements.borrow().get(key).cloned()
    }

    pub fn realize_all(&mut self) {
        if let Some(rest) = self.rest.take() {
            self.elements.borrow_mut().extend(rest);
        }
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

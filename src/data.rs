use std::collections::HashMap;

#[derive(Debug)]
pub enum Value {
    Number(u32),
    String(String),
    List(List),
    Map(Map),
}

/// Lazily evaluated list
pub struct List {
    pub elements: Vec<Value>,
    pub rest: Option<Box<dyn Iterator<Item = Value>>>,
}

/// Lazily evaluated map
pub struct Map {
    pub elements: HashMap<String, Value>,
    pub rest: Option<Box<dyn Iterator<Item = (String, Value)>>>,
}

impl List {
    pub fn get(&mut self, n: usize) -> Option<&Value> {
        self.realize(n);
        self.elements.get(n)
    }

    pub fn realize_all(&mut self) {
        if let Some(rest) = self.rest.take() {
            self.elements.extend(rest);
        }
    }

    fn realize(&mut self, n: usize) {
        let elements_needed = n - self.elements.len();
        if elements_needed > 0 {
            if let Some(rest) = self.rest.as_mut() {
                self.elements.extend(rest.take(elements_needed));
            }
        }
    }
}

impl std::fmt::Debug for List {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("List")
            .field("elements", &self.elements)
            .field("lazy_extra", &self.rest.is_some())
            .finish()
    }
}

impl Map {
    pub fn get(&mut self, key: &str) -> Option<&Value> {
        self.realize_all();
        self.elements.get(key)
    }

    pub fn realize_all(&mut self) {
        if let Some(rest) = self.rest.take() {
            self.elements.extend(rest);
        }
    }
}

impl std::fmt::Debug for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Map")
            .field("elements", &self.elements)
            .field("lazy_extra", &self.rest.is_some())
            .finish()
    }
}

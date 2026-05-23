use super::Value;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct Tuple(pub Vec<Value>);

impl Tuple {
    pub fn new() -> Self {
        Tuple(Vec::new())
    }

    pub fn from_values(values: Vec<Value>) -> Self {
        Tuple(values)
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for Tuple {
    type Target = Vec<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

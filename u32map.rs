//! HashMap that assigns items by ID for later retrieval

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct U32Map<T> {
    map: HashMap<u32, T>,
    val: u32,
}

impl<T> U32Map<T> {
    pub fn new() -> Self {
        U32Map {
            map: HashMap::new(),
            val: u32::max_value(),
        }
    }

    pub fn insert(&mut self, value: T) -> Option<u32> {
        let initial = self.val;
        self.val = self.val.wrapping_add(1);
        while self.map.contains_key(&self.val) && self.val != initial {
            self.val = self.val.wrapping_add(1);
        }
        if self.val == initial {
            None
        } else {
            self.map.insert(self.val, value);
            Some(self.val)
        }
    }

    pub fn get(&self, key: &u32) -> Option<&T> {
        self.map.get(key)
    }
}

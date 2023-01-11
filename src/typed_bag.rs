use std::collections::BTreeMap;

use crate::{
    bag::ValueBag,
    env::{Environment, EvalError},
    query::{Predicate, ProjectionQuery, DeletionQuery, UpdateQuery, check_value},
    value::Value,
};

pub struct TypedBag<'i, 's, 'v> {
    bag: ValueBag<'s, 'v>,
    guard: Predicate<'s>,
    env: Environment<'i, 's, 'v>,
}

impl<'i, 's, 'v> TypedBag<'i, 's, 'v> {
    pub fn new(guard: Predicate<'s>) -> Self {
        Self {
            bag: ValueBag::new(),
            guard,
            env: Environment {
                bindings: BTreeMap::new(),
            },
        }
    }

    pub fn insert(&mut self, value: &Value<'s, 'v>) -> bool {
        if check_value(&self.env, &self.guard, value) {
            self.bag.insert(value);
            true
        } else {
            false
        }
    }

    pub fn pop(&mut self, value: &Value<'s, 'v>) -> bool {
        self.bag.pop(value)
    }

    pub fn query<'e, 'x: 'e>(
        &'x self,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        self.bag.query(env, query)
    }

    pub fn delete<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>,
    ) -> usize {
        self.bag.delete(env, deletion)
    }
    
    pub(crate) fn update<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e UpdateQuery<'s>,
    ) -> usize {
        self.bag.checked_update(env, deletion, 
            &self.guard)
    }

    pub fn iter<'x>(&'x self) -> std::slice::Iter<'x, std::borrow::Cow<'v, Value<'s, 'v>>> {
        self.bag.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.bag.len()
    }
}

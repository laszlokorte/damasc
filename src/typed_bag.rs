use std::collections::BTreeMap;

use crate::{
    bag::ValueBag,
    env::{Environment, EvalError},
    matcher::Matcher,
    query::{Predicate, ProjectionQuery, DeletionQuery, UpdateQuery},
    value::Value,
};

pub struct TypedBag<'i, 's, 'v> {
    bag: ValueBag<'s, 'v>,
    guard: Predicate<'s>,
    env: Environment<'i, 's, 'v>,
}

pub(crate) fn check_value<'s,'v>(env: &Environment<'_, 's, 'v>, pred: &Predicate<'s>, val: &Value<'s, 'v>) -> bool {
    let mut matcher = Matcher {
        env,
        bindings: BTreeMap::new(),
    };

    let Ok(()) = matcher.match_pattern(&pred.pattern, val) else {
        return false;
    };

    let local_env = matcher.make_env();

    let Ok(Value::Boolean(true)) = local_env.eval_expr(&pred.guard) else {
        return false;
    };

    true
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

use std::collections::BTreeMap;

use crate::{
    bag::ValueBag,
    env::{Environment, EvalError},
    matcher::Matcher,
    query::{Predicate, Query},
    value::Value,
};

pub(crate) struct TypedBag<'i, 's, 'v> {
    bag: ValueBag<'s, 'v>,
    guard: Predicate<'s>,
    env: Environment<'i, 's, 'v>,
}

impl<'i, 's, 'v> TypedBag<'i, 's, 'v> {
    pub(crate) fn new(guard: Predicate<'s>) -> Self {
        Self {
            bag: ValueBag::new(),
            guard,
            env: Environment {
                bindings: BTreeMap::new(),
            },
        }
    }

    pub(crate) fn insert(&mut self, value: &Value<'s, 'v>) -> bool {
        if let Some(limit) = self.guard.limit && limit <= self.bag.len() {
            return false;
        }
        let mut matcher = Matcher {
            env: &self.env,
            bindings: BTreeMap::new(),
        };
        let Ok(()) = matcher.match_pattern(&self.guard.pattern, value) else {
            return false;
        };

        let local_env = matcher.make_env();

        let Ok(Value::Boolean(true)) = local_env.eval_expr(&self.guard.guard) else {
            return false;
        };

        self.bag.insert(value)
    }

    pub(crate) fn pop(&mut self, value: &Value<'s, 'v>) -> bool {
        self.bag.pop(value)
    }

    pub(crate) fn query<'e, 'x: 'e>(
        &'x self,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e Query<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        self.bag.query(env, query)
    }

    pub(crate) fn delete<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        predicate: &'e Predicate<'s>,
    ) -> usize {
        self.bag.delete(env, predicate)
    }

    pub(crate) fn iter<'x>(&'x self) -> std::slice::Iter<'x, std::borrow::Cow<'v, Value<'s, 'v>>> {
        self.bag.iter()
    }
}

use std::collections::BTreeMap;

use crate::{
    bag::ValueBag,
    env::{Environment, EvalError},
    query::{Predicate, ProjectionQuery, DeletionQuery, UpdateQuery, check_value, TransfereQuery},
    value::Value, matcher::Matcher,
};

pub struct TypedBag<'i, 's, 'v> {
    bag: ValueBag<'s, 'v>,
    pub(crate) guard: Predicate<'s>,
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

pub(crate) struct TypedTransfer<'x, 'i, 's, 'v> {
    source: &'x mut TypedBag<'i, 's, 'v>,
    target: &'x mut TypedBag<'i, 's, 'v>,
}
impl<'x, 'i, 's, 'v> TypedTransfer<'x, 'i, 's, 'v> {
    pub(crate) fn new(source: &'x mut TypedBag<'i, 's, 'v>, target: &'x mut TypedBag<'i, 's, 'v>) -> Self {
        Self {
            source,
            target,
        }
    }

    pub(crate) fn transfer<'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        transfer: &'e TransfereQuery<'s>,
    ) -> usize {
        let mut counter = 0;
        let mut matcher = Matcher {
            env: &env.clone(),
            bindings: BTreeMap::new(),
        };

        self.source.bag.items.retain(|item| {
            if let Some(limit) = transfer.predicate.limit {
                if limit <= counter {
                    return true;
                }
            }

            matcher.clear();

            if !matches!(
                matcher.match_pattern(&transfer.predicate.pattern, item.as_ref()),
                Ok(())
            ) {
                true
            } else {
                let mut env = env.clone();
                matcher.apply_to_env(&mut env);
                let shall_transfer =
                    matches!(env.eval_expr(&transfer.predicate.guard), Ok(Value::Boolean(true)));
                if shall_transfer {
                    let Ok(target_value) = env.eval_expr(&transfer.projection) else {
                        return true;
                    };
                    if self.target.insert(&target_value) {
                        counter += 1;
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
        });

        counter
    }
}

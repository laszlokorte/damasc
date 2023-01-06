use std::{collections::{BTreeMap, HashSet}, borrow::Cow};

use gen_iter::gen_iter;
use multiset::HashMultiSet;

use crate::{
    env::{Environment, EvalError},
    matcher::Matcher,
    query::{Predicate, Query},
    value::Value,
};

pub(crate) struct ValueBag<'s, 'v> {
    items: HashMultiSet<Cow<'v, Value<'s, 'v>>>,
}

impl<'s, 'v> ValueBag<'s, 'v> {
    pub(crate) fn new() -> Self {
        Self {
            items: HashMultiSet::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: &Value<'s, 'v>) {
        self.items.insert(Cow::Owned(value.clone()));
    }

    pub(crate) fn count(&mut self, value: &Value<'s, 'v>) -> usize {
        self.items.count_of(&Cow::Borrowed(value))
    }

    pub(crate) fn pop(&mut self, value: &Value<'s, 'v>) -> bool {
        self.items.remove(&Cow::Owned(value.clone()))
    }

    pub(crate) fn query<'e, 'x: 'e, 'i>(
        &'x self,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e Query<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        gen_iter!(move {
            let mut count = 0;
            for item in self.items.iter() {
                let mut matcher = Matcher {
                    env: &env.clone(),
                    bindings: BTreeMap::new(),
                };
                if let Ok(()) = matcher.match_pattern(&query.predicate.pattern, item.as_ref()) {
                    let mut env = env.clone();
                    matcher.apply_to_env(&mut env);
                    if let Ok(Value::Boolean(true)) = env.eval_expr(&query.predicate.guard) {
                        yield env.eval_expr(&query.projection);
                        count+=1;
                        if let Some(l) = query.predicate.limit && count >= l {
                            break;
                        }
                    }
                }
            }
        })
    }

    pub(crate) fn delete<'e, 'x: 'e, 'i>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        predicate: &'e Predicate<'s>,
    ) {
        let to_delete: HashSet<_> = self
            .items
            .distinct_elements()
            .into_iter()
            .filter(|&item| {
                let mut matcher = Matcher {
                    env: &env.clone(),
                    bindings: BTreeMap::new(),
                };

                if !matches!(
                    matcher.match_pattern(&predicate.pattern, item.as_ref()),
                    Ok(())
                ) {
                    false
                } else {
                    let mut env = env.clone();
                    matcher.apply_to_env(&mut env);
                    matches!(env.eval_expr(&predicate.guard), Ok(Value::Boolean(true)))
                }
            })
            .cloned()
            .collect();

        if let Some(mut remaining) = predicate.limit {
            for d in to_delete {
                remaining -= self.items.remove_times(&d, remaining);

                if remaining == 0 {
                    break;
                }
            }
        } else {
            for d in to_delete {
                self.items.remove_all(&d);
            }
        }
    }
}

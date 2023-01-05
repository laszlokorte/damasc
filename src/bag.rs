use std::collections::{BTreeMap, HashSet};

use multiset::HashMultiSet;
use gen_iter::gen_iter;

use crate::{value::Value, pattern::Pattern, matcher::Matcher, env::{Environment, EvalError}, query::Query};

pub(crate) struct ValueBag<'s,'v> {
    items: HashMultiSet<Value<'s,'v>>
}

impl<'s,'v> ValueBag<'s,'v> {
    pub(crate) fn new() -> Self {
        Self {
            items: HashMultiSet::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: &Value<'s,'v>) {
        self.items.insert(value.clone());
    }

    pub(crate) fn count(&mut self, value: &Value<'s,'v>) -> usize {
        self.items.count_of(value)
    }

    pub(crate) fn pop(&mut self, value: &Value<'s,'v>) -> bool {
        self.items.remove(value)
    }

    pub(crate) fn query<'e, 'x:'e,'i>(&'x self, env: &'e Environment<'i, 's, 'v>, query: &'e Query<'s>) -> impl Iterator<Item = Result<Value<'s,'v>, EvalError>> + 'e {
        gen_iter!(move {
            for item in self.items.iter() {
                let mut matcher = Matcher {
                    env: &env.clone(),
                    bindings: BTreeMap::new(),
                };
                if let Ok(()) = matcher.match_pattern(&query.predicate, item.clone()) {
                    let mut env = env.clone();
                    matcher.apply_to_env(&mut env);
                    yield env.eval_expr(&query.projection);
                }
            }
        })
    }

    pub(crate) fn delete<'e, 'x:'e,'i>(&'x mut self, env: &'e Environment<'i, 's, 'v>, pattern: &'e Pattern<'s>) {
        let to_delete : HashSet<_> = self.items.distinct_elements().into_iter().filter(|&item| {
            let mut matcher = Matcher {
                env: &env.clone(),
                bindings: BTreeMap::new(),
            };

            matches!(matcher.match_pattern(pattern, item.clone()), Ok(()))
        }).cloned().collect();

        for d in to_delete {
            self.items.remove_all(&d);
        }
    }
}
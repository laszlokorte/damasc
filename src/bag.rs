use std::{collections::{BTreeMap}, borrow::Cow};

use gen_iter::gen_iter;

use crate::{
    env::{Environment, EvalError},
    matcher::Matcher,
    query::{Predicate, Query, CrossQuery},
    value::Value, 
};

pub(crate) struct ValueBag<'s, 'v> {
    items: Vec<Cow<'v, Value<'s, 'v>>>,
}

impl<'s, 'v> ValueBag<'s, 'v> {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: &Value<'s, 'v>) {
        self.items.push(Cow::Owned(value.clone()));
    }

    pub(crate) fn pop(&mut self, value: &Value<'s, 'v>) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.as_ref() == value) {
            self.items.swap_remove(pos); 
            true
        } else {
            false
        }
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

    pub(crate) fn cross_query<'e, 'x: 'e, 'i>(
        &'x self,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e CrossQuery<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        gen_iter!(move {
            let mut count = 0;
            'outer: for (idx_a, item_a) in self.items.iter().enumerate() {
                let mut matcher = Matcher {
                    env,
                    bindings: BTreeMap::new(),
                };
                if let Ok(()) = matcher.match_pattern(&query.predicate.patterns[0], item_a.as_ref()) {
                    for (idx_b, item_b) in self.items.iter().enumerate() {
                        if !query.outer && idx_a == idx_b {
                            continue;
                        }

                        let mut matcher = matcher.clone();

                        if let Ok(()) = matcher.match_pattern(&query.predicate.patterns[1], item_b.as_ref()) {
                            let mut env = env.clone();
                            matcher.apply_to_env(&mut env);
                            if let Ok(Value::Boolean(true)) = env.eval_expr(&query.predicate.guard) {
                                yield env.eval_expr(&query.projection);
                                count+=1;
                                if let Some(l) = query.predicate.limit && count >= l {
                                    break 'outer;
                                }
                            }
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
        let mut counter = 0;
        let mut matcher = Matcher {
            env: &env.clone(),
            bindings: BTreeMap::new(),
        };

        self.items.retain(|item| {
            if let Some(limit) = predicate.limit && limit <= counter {
                return true;
            }

            matcher.clear();

            if !matches!(
                matcher.match_pattern(&predicate.pattern, item.as_ref()),
                Ok(())
            ) {
                true
            } else {
                let mut env = env.clone();
                matcher.apply_to_env(&mut env);
                let should_delete = matches!(env.eval_expr(&predicate.guard), Ok(Value::Boolean(true)));
                if should_delete {
                    counter += 1;
                    false
                } else {
                    true
                }
            }
        });
    }

    pub(crate) fn iter<'x>(&'x self) -> std::slice::Iter<'x, std::borrow::Cow<'v, Value<'s, 'v>>> {
        self.items.iter()
    }
}

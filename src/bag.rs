use std::{borrow::Cow, collections::BTreeMap};

use gen_iter::gen_iter;

const MAX_JOIN_SIZE: usize = 6;

use crate::{
    env::{Environment, EvalError},
    matcher::Matcher,
    pattern::Pattern,
    query::{ProjectionQuery, DeletionQuery, UpdateQuery, Predicate},
    value::Value, typed_bag,
};

pub(crate) struct ValueBag<'s, 'v> {
    pub(crate) items: Vec<Cow<'v, Value<'s, 'v>>>,
}

impl<'s, 'v> ValueBag<'s, 'v> {
    pub(crate) fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub(crate) fn insert(&mut self, value: &Value<'s, 'v>) -> bool {
        self.items.push(Cow::Owned(value.clone()));
        true
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
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
        query: &'e ProjectionQuery<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        gen_iter!(move {
            let matcher = Matcher {
                env,
                bindings: BTreeMap::new(),
            };
            let mut count = 0;

            if query.predicate.patterns.len() > MAX_JOIN_SIZE {
                yield Err(EvalError::Overflow);
                return;
            }

            for mut m in self.cross_query_helper(query.outer, 0, [0;MAX_JOIN_SIZE], matcher, &query.predicate.patterns) {
                let mut env = env.clone();
                m.apply_to_env(&mut env);
                if let Ok(Value::Boolean(true)) = env.eval_expr(&query.predicate.guard) {
                    yield env.eval_expr(&query.projection);
                    count+=1;
                    if let Some(l) = query.predicate.limit {
                        if count >= l {
                            break;
                        }
                    }
                }
            }
        })
    }

    fn cross_query_helper<'e, 'x: 'e, 'i>(
        &'x self,
        outer: bool,
        depth: usize,
        skip: [usize; MAX_JOIN_SIZE],
        matcher: Matcher<'i, 's, 'v, 'e>,
        patterns: &'e [Pattern<'s>],
    ) -> Box<dyn Iterator<Item = Matcher<'i, 's, 'v, 'e>> + 'e> {
        let Some(pattern) = patterns.get(0) else {
            return Box::new(Some(matcher.clone()).into_iter())
        };

        Box::new(gen_iter!(move {
            for (idx, item) in self.items.iter().enumerate() {
                if !outer && skip[0..depth].contains(&idx) {
                    continue;
                }

                let mut m = matcher.clone();
                let Ok(()) = m.match_pattern(pattern, item) else {
                    continue;
                };

                let mut skip_x = skip;
                skip_x[depth] = idx;

                for mm in self.cross_query_helper(outer, depth+1, skip_x, m, &patterns[1..]) {
                    yield mm;
                }
            }
        }))
    }

    pub(crate) fn delete<'e, 'x: 'e, 'i>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>,
    ) -> usize {
        let mut counter = 0;
        let mut matcher = Matcher {
            env: &env.clone(),
            bindings: BTreeMap::new(),
        };

        self.items.retain(|item| {
            if let Some(limit) = deletion.predicate.limit {
                if limit <= counter {
                    return true;
                }
            }

            matcher.clear();

            if !matches!(
                matcher.match_pattern(&deletion.predicate.pattern, item.as_ref()),
                Ok(())
            ) {
                true
            } else {
                let mut env = env.clone();
                matcher.apply_to_env(&mut env);
                let should_delete =
                    matches!(env.eval_expr(&deletion.predicate.guard), Ok(Value::Boolean(true)));
                if should_delete {
                    counter += 1;
                    false
                } else {
                    true
                }
            }
        });

        counter
    }

    pub(crate) fn update<'e, 'x: 'e, 'i>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>,
    ) -> usize {
        self.checked_update(env, update,  &Predicate::any())
    }

    pub(crate) fn checked_update<'e, 'x: 'e, 'i>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>,
        post_predicate: &Predicate<'s>
    ) -> usize
      {
        let mut counter = 0;

        let mut matcher = Matcher {
            env: &env.clone(),
            bindings: BTreeMap::new(),
        };

        for item in &mut self.items { 
            if let Some(limit) = update.predicate.limit {
                if limit <= counter {
                    break;
                }
            }

            matcher.clear();

            if !matches!(
                matcher.match_pattern(&update.predicate.pattern, item.as_ref()),
                Ok(())
            ) {
                continue;
            } else {
                let mut env = env.clone();
                matcher.apply_to_env(&mut env);
                let should_delete =
                    matches!(env.eval_expr(&update.predicate.guard), Ok(Value::Boolean(true)));
                if should_delete {
                    let Ok(val) = env.eval_expr(&update.projection) else {
                        continue;
                    };
                    if typed_bag::check_value(&env, post_predicate, &val) {
                        *item = Cow::Owned(val);
                        counter += 1;
                    }
                } else {
                    continue;
                }
            }

        }
        counter
    }

    pub(crate) fn iter<'x>(&'x self) -> std::slice::Iter<'x, std::borrow::Cow<'v, Value<'s, 'v>>> {
        self.items.iter()
    }
}

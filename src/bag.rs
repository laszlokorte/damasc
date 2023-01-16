use std::{borrow::Cow, collections::BTreeMap};

use gen_iter::gen_iter;

const MAX_JOIN_SIZE: usize = 6;

use crate::{
    env::{Environment, EvalError},
    expression::Expression,
    matcher::Matcher,
    pattern::Pattern,
    query::{
        check_value, DeletionQuery, Insertion, Predicate, ProjectionQuery, TransferQuery,
        UpdateQuery,
    },
    value::Value,
};

#[derive(Clone)]
pub struct ValueBag<'i, 's, 'v> {
    pub(crate) items: Vec<Cow<'v, Value<'s, 'v>>>,
    pub(crate) guard: Predicate<'s>,
    env: Environment<'i, 's, 'v>,
}

pub(crate) enum InsertionResult {
    Success(usize),
    GuardError,
    EvalError,
}
pub(crate) enum DeletionResult {
    Success(usize),
    EvalError,
}
pub(crate) enum UpdateResult {
    Success(usize),
    GuardError,
    EvalError,
}
pub(crate) enum TransferResult {
    Success(usize),
    GuardError,
    EvalError,
}

impl<'i, 's, 'v> ValueBag<'i, 's, 'v> {
    pub fn new(guard: Predicate<'s>) -> Self {
        Self {
            items: vec![],
            guard,
            env: Environment {
                bindings: BTreeMap::new(),
            },
        }
    }

    pub(crate) fn insert<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        insertion: &'e Insertion<'s>,
    ) -> InsertionResult {
        let mut counter = 0;
        for expr in &insertion.expressions.expressions {
            match self.insert_one(env, expr) {
                InsertionResult::Success(_) => counter += 1,
                err => return err,
            }
        }

        InsertionResult::Success(counter)
    }

    pub(crate) fn insert_one<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        expression: &'e Expression<'s>,
    ) -> InsertionResult {
        let eval_result = env.eval_expr(expression);

        if let Ok(value) = eval_result {
            if check_value(&self.env, &self.guard, &value, self.len()) {
                self.items.push(Cow::Owned(value.clone()));
                InsertionResult::Success(1)
            } else {
                InsertionResult::GuardError
            }
        } else {
            InsertionResult::EvalError
        }
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


    pub(crate) fn query<'e, 'x: 'e>(
        &'x self,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e {
        gen_iter!(move {
            let matcher = Matcher::new(&env);
            let mut count = 0;

            if query.predicate.patterns.len() > MAX_JOIN_SIZE {
                yield Err(EvalError::Overflow);
                return;
            }

            let duplicates = Vec::with_capacity(query.predicate.patterns.len());

            for m in self.cross_query_helper(query.outer, duplicates, matcher, &query.predicate.patterns) {
                let mut env = env.clone();
                m.into_env().merge(&mut env);
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

    pub(crate) fn cross_query_helper<'e, 'x: 'e, 'dup>(
        &'x self,
        outer: bool,
        mut skip: Vec<usize>,
        matcher: Matcher<'i, 's, 'v, 'e>,
        patterns: &'e [Pattern<'s>],
    ) -> Box<dyn Iterator<Item = Matcher<'i, 's, 'v, 'e>> + 'e> {
        let Some(pattern) = patterns.get(0) else {
            return Box::new(Some(matcher.clone()).into_iter())
        };

        Box::new(gen_iter!(move {
            for (idx, item) in self.items.iter().enumerate() {
                if !outer && skip.contains(&idx) {
                    continue;
                }

                let mut m = matcher.clone();
                let Ok(()) = m.match_pattern(pattern, item) else {
                    continue;
                };

                skip.push(idx);
                for mm in self.cross_query_helper(outer, skip.clone(), m, &patterns[1..]) {
                    yield mm;
                }
                skip.pop();
            }
        }))
    }

    pub(crate) fn delete<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>,
    ) -> DeletionResult {
        let mut counter = 0;
        let mut eval_error = false;
        let mut matcher = Matcher::new(&env);

        self.items.retain(|item| {
            if eval_error {
                return true;
            }
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
                matcher.local_env.clone().merge(&mut env);
                let Ok(Value::Boolean(shall_delete)) = env.eval_expr(&deletion.predicate.guard) else {
                    eval_error = true;
                    return true;
                };
                if shall_delete {
                    counter += 1;
                    false
                } else {
                    true
                }
            }
        });

        if eval_error {
            DeletionResult::EvalError
        } else {
            DeletionResult::Success(counter)
        }
    }
    pub(crate) fn update<'e, 'x: 'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>,
    ) -> UpdateResult {
        let mut counter = 0;

        let mut matcher = Matcher::new(&env);

        let bag_size = self.items.len();

        for item in &mut self.items {
            if let Some(limit) = update.predicate.limit {
                if limit <= counter {
                    return UpdateResult::Success(counter);
                }
            }

            matcher.clear();

            if matches!(
                matcher.match_pattern(&update.predicate.pattern, item.as_ref()),
                Ok(())
            ) {
                continue;
            } else {
                let mut env = env.clone();
                matcher.local_env.clone().merge(&mut env);
                let Ok(Value::Boolean(should_update)) = env.eval_expr(&update.predicate.guard) else {
                    return UpdateResult::EvalError;
                };

                if should_update {
                    let Ok(val) = env.eval_expr(&update.projection) else {
                        return UpdateResult::EvalError;
                    };
                    if check_value(&env, &self.guard, &val, bag_size) {
                        *item = Cow::Owned(val);
                        counter += 1;
                    } else {
                        return UpdateResult::GuardError;
                    }
                } else {
                    continue;
                }
            }
        }
        UpdateResult::Success(counter)
    }

    pub(crate) fn iter<'x>(&'x self) -> std::slice::Iter<'x, std::borrow::Cow<'v, Value<'s, 'v>>> {
        self.items.iter()
    }
}

pub(crate) struct ValueBagTransfer<'x, 'i, 's, 'v> {
    source: &'x mut ValueBag<'i, 's, 'v>,
    target: &'x mut ValueBag<'i, 's, 'v>,
}
impl<'x, 'i, 's, 'v> ValueBagTransfer<'x, 'i, 's, 'v> {
    pub(crate) fn new(
        source: &'x mut ValueBag<'i, 's, 'v>,
        target: &'x mut ValueBag<'i, 's, 'v>,
    ) -> Self {
        Self { source, target }
    }

    pub(crate) fn transfer<'e>(
        &'x mut self,
        env: &'e Environment<'i, 's, 'v>,
        transfer: &'e TransferQuery<'s>,
    ) -> TransferResult {
        let mut counter: usize = 0;
        let mut short_circuit: Option<TransferResult> = None;
        let mut matcher = Matcher::new(&env);

        self.source.items.retain(|item| {
            if short_circuit.is_some() {
                return true;
            }

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
                matcher.local_env.clone().merge(&mut env);
                let Ok(Value::Boolean(shall_transfer)) = env.eval_expr(&transfer.predicate.guard) else {
                    short_circuit = Some(TransferResult::EvalError);
                    return true;
                };
                if shall_transfer {
                    match self.target.insert_one(&env, &transfer.projection) {
                        InsertionResult::Success(_) => {
                            counter += 1;
                            false
                        },
                        InsertionResult::EvalError => {
                            short_circuit = Some(TransferResult::EvalError);
                            true
                        }
                        InsertionResult::GuardError => {
                            short_circuit = Some(TransferResult::GuardError);
                            true
                        }
                    }
                } else {
                    true
                }
            }
        });

        short_circuit.unwrap_or(TransferResult::Success(counter))
    }
}



struct BagQueryIterator<'dup, 'i, 's, 'v, 'e> {
    duplicates: &'dup mut Vec<usize>,
    outer: bool,
    matcher: Matcher<'i, 's, 'v, 'e>,
    patterns: &'e [Pattern<'s>],
}

impl<'dup, 'i, 's, 'v, 'e> Iterator for BagQueryIterator<'dup, 'i, 's, 'v, 'e> {
    type Item  = Matcher<'i, 's, 'v, 'e>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
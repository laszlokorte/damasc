use std::collections::BTreeMap;

use crate::{expression::Expression, pattern::Pattern, literal::Literal, matcher::Matcher, env::Environment, value::Value};

#[derive(Clone)]
pub struct ProjectionQuery<'s> {
    pub outer: bool,
    pub predicate: CrossPredicate<'s>,
    pub projection: Expression<'s>,
}

#[derive(Clone)]
pub struct Predicate<'s> {
    pub pattern: Pattern<'s>,
    pub guard: Expression<'s>,
    pub limit: Option<usize>,
}

impl<'s> Predicate<'s>  {
    pub(crate) fn any() -> Self {
        Self {
            pattern: Pattern::Discard,
            guard: Expression::Literal(Literal::Boolean(true)),
            limit: None,
        }
    }
}

#[derive(Clone)]
pub struct CrossPredicate<'s> {
    pub patterns: Vec<Pattern<'s>>,
    pub guard: Expression<'s>,
    pub limit: Option<usize>,
}



#[derive(Clone)]
pub struct DeletionQuery<'s> {
    pub predicate: Predicate<'s>,
}

#[derive(Clone)]
pub struct UpdateQuery<'s> {
    pub predicate: Predicate<'s>,
    pub projection: Expression<'s>,
}


#[derive(Clone)]
pub struct TransfereQuery<'s> {
    pub predicate: Predicate<'s>,
    pub projection: Expression<'s>,
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
use std::collections::BTreeMap;

use crate::{expression::{Expression, ExpressionSet}, pattern::Pattern, literal::Literal, matcher::Matcher, env::Environment, value::Value};

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

    fn is_any(&self) -> bool {
        matches!(self.pattern, Pattern::Discard) && matches!(self.guard, Expression::Literal(Literal::Boolean(true)))
    }
}

impl<'s> std::fmt::Display for Predicate<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.is_any() {
            write!(f, "{}", self.pattern)?;
            write!(f, " where {}", self.guard)?;

            if let Some(l) = self.limit {
                write!(f, " limit {l}")?;
            }
        } else if let Some(l) = self.limit {
            write!(f, "limit {l}")?;
        } else {
            write!(f,"none")?;
        }

        Ok(())
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
pub struct TransferQuery<'s> {
    pub predicate: Predicate<'s>,
    pub projection: Expression<'s>,
}


pub(crate) fn check_value<'s,'v>(env: &Environment<'_, 's, 'v>, pred: &Predicate<'s>, val: &Value<'s, 'v>, count: usize) -> bool {
    if let Some(l) = pred.limit {
        if l <= count {
            return false;
        }
    }
    
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


#[derive(Clone)]
pub struct Insertion<'s> {
    pub(crate) expressions: ExpressionSet<'s>
}
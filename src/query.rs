use crate::{expression::Expression, pattern::Pattern, literal::Literal};

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
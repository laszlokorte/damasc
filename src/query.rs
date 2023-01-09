use crate::{expression::Expression, pattern::Pattern};

#[derive(Clone)]
pub struct Query<'s> {
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

#[derive(Clone)]
pub struct CrossPredicate<'s> {
    pub patterns: Vec<Pattern<'s>>,
    pub guard: Expression<'s>,
    pub limit: Option<usize>,
}

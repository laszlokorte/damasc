use crate::{expression::Expression, pattern::Pattern};

#[derive(Clone)]
pub(crate) struct Query<'s> {
    pub(crate) outer: bool,
    pub(crate) predicate: CrossPredicate<'s>,
    pub(crate) projection: Expression<'s>,
}

#[derive(Clone)]
pub(crate) struct Predicate<'s> {
    pub(crate) pattern: Pattern<'s>,
    pub(crate) guard: Expression<'s>,
    pub(crate) limit: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct CrossPredicate<'s> {
    pub(crate) patterns: Vec<Pattern<'s>>,
    pub(crate) guard: Expression<'s>,
    pub(crate) limit: Option<usize>,
}

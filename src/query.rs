use crate::{expression::Expression, pattern::Pattern};

#[derive(Clone)]
pub(crate) struct Query<'s> {
    pub(crate) predicate: Predicate<'s>,
    pub(crate) projection: Expression<'s>,
}

#[derive(Clone)]
pub(crate) struct Predicate<'s> {
    pub(crate) pattern: Pattern<'s>,
    pub(crate) guard: Expression<'s>,
    pub(crate) limit: Option<usize>,
}

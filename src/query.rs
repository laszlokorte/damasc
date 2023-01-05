use crate::{pattern::Pattern, expression::Expression};

#[derive(Clone)]
pub(crate) struct Query<'s> {
    pub(crate) predicate: Pattern<'s>,
    pub(crate) projection: Expression<'s>,
}
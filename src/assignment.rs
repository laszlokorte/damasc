use crate::{pattern::Pattern, expression::Expression};

#[derive(Clone)]
pub(crate) struct Assignment<'a,'b> {
    pub(crate) pattern: Pattern<'a>, 
    pub(crate) expression: Expression<'b>
}

pub(crate) struct AssgmentSet<'a,'b> {
    pub(crate) assignments: [Assignment<'a,'b>]
}
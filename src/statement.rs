use crate::{expression::Expression, pattern::Pattern};

#[derive(Clone)]
pub(crate) enum Statement<'a, 'b> {
    Clear,
    Inspect(Expression<'b>),
    Format(Expression<'b>),
    Eval(Expression<'b>),
    Literal(Expression<'b>),
    Pattern(Pattern<'b>),
    Assign(Pattern<'a>, Expression<'b>),
    Match(Pattern<'a>, Expression<'b>),
}

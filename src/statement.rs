use crate::{expression::Expression, pattern::Pattern, query::{Query, Predicate}};

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
    Insert(Expression<'b>),
    Pop(Expression<'b>),
    Query(Query<'a>),
    Deletion(Predicate<'a>),
}

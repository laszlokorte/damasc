use std::borrow::Cow;

use crate::{
    expression::Expression,
    pattern::Pattern,
    query::{Predicate, Query}, identifier::Identifier, assignment::Assignment,
};

#[derive(Clone)]
pub(crate) enum Statement<'a, 'b> {
    Clear,
    Exit,
    Help,
    Inspect(Expression<'b>),
    Format(Expression<'b>),
    Eval(Expression<'b>),
    Literal(Expression<'b>),
    Pattern(Pattern<'b>),
    Assign(Assignment<'a,'b>),
    Match(Assignment<'a,'b>),
    Insert(Vec<Expression<'b>>),
    Pop(Expression<'b>),
    Query(Query<'a>),
    Deletion(Predicate<'a>),
    Import(Cow<'b, str>),
    Export(Cow<'b, str>),
    UseBag(Identifier<'b>, Option<Predicate<'b>>),
    TellBag,
}

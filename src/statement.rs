use std::borrow::Cow;

use crate::{
    assignment::AssignmentSet,
    expression::{Expression, ExpressionSet},
    identifier::Identifier,
    pattern::Pattern,
    query::{Predicate, Query},
};

#[derive(Clone)]
pub enum Statement<'a, 'b> {
    Noop,
    Clear,
    Exit,
    Help,
    Inspect(Expression<'b>),
    Format(Expression<'b>),
    Eval(ExpressionSet<'b>),
    Literal(Expression<'b>),
    Pattern(Pattern<'b>),
    AssignSet(AssignmentSet<'a, 'b>),
    MatchSet(AssignmentSet<'a, 'b>),
    Insert(Vec<Expression<'b>>),
    Pop(Expression<'b>),
    Query(Query<'a>),
    Deletion(Predicate<'a>),
    Import(Cow<'b, str>),
    Export(Cow<'b, str>),
    UseBag(Identifier<'b>, Option<Predicate<'b>>),
    TellBag,
}

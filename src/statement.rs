use std::borrow::Cow;

use crate::{
    assignment::AssignmentSet,
    expression::{Expression, ExpressionSet},
    identifier::Identifier,
    pattern::Pattern,
    query::{Predicate, ProjectionQuery, DeletionQuery, UpdateQuery, TransfereQuery},
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
    Query(ProjectionQuery<'a>),
    Deletion(DeletionQuery<'a>),
    Update(UpdateQuery<'a>),
    Move(Identifier<'b>, Identifier<'b>, TransfereQuery<'a>),
    Import(Cow<'b, str>),
    Export(Cow<'b, str>),
    UseBag(Identifier<'b>, Option<Predicate<'b>>),
    TellBag,
    ListBags,
}

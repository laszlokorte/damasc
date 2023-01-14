use std::borrow::Cow;

use crate::{
    assignment::AssignmentSet,
    expression::{Expression, ExpressionSet},
    identifier::Identifier,
    pattern::Pattern,
    query::{DeletionQuery, Insertion, Predicate, ProjectionQuery, TransferQuery, UpdateQuery}, graph::Connection,
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
    Insert(Insertion<'b>),
    Pop(Expression<'b>),
    Query(ProjectionQuery<'a>),
    Deletion(DeletionQuery<'a>),
    Update(UpdateQuery<'a>),
    Move(Identifier<'b>, TransferQuery<'a>),
    Import(Cow<'b, str>),
    Export(Cow<'b, str>),
    LoadBundle(Cow<'b, str>),
    UseBag(Identifier<'b>, Option<Predicate<'b>>),
    DropBag(Identifier<'b>),
    Connect(Connection<'b>),
    Disconnect(usize),
    ListConnections,
    TellBag,
    ListBags,
}

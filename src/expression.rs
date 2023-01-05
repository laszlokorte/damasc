use std::borrow::Cow;

use crate::identifier::Identifier;
use crate::value::ValueType;

#[derive(Clone, Debug)]
pub(crate) enum Expression<'s> {
    Array(ArrayExpression<'s>),
    Binary(BinaryExpression<'s>),
    Identifier(Identifier<'s>),
    Literal(Literal<'s>),
    Logical(LogicalExpression<'s>),
    Member(MemberExpression<'s>),
    Object(ObjectExpression<'s>),
    Unary(UnaryExpression<'s>),
    Call(CallExpression<'s>),
}

impl std::fmt::Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type ArrayExpression<'a> = Vec<ArrayItem<'a>>;

#[derive(Clone, Debug)]
pub(crate) enum ArrayItem<'a> {
    Single(Expression<'a>),
    Spread(Expression<'a>),
}


pub(crate) type ObjectExpression<'a> = Vec<ObjectProperty<'a>>;

#[derive(Clone, Debug)]
pub(crate) enum ObjectProperty<'a> {
    Single(Identifier<'a>),
    Property(Property<'a>),
    Spread(Expression<'a>),
}

#[derive(Clone, Debug)]
pub(crate) struct Property<'a> {
    pub(crate) key: PropertyKey<'a>,
    pub(crate) value: Expression<'a>,
}

#[derive(Clone, Debug)]
pub(crate) enum PropertyKey<'a> {
    Identifier(Identifier<'a>),
    Expression(Expression<'a>),
}

#[derive(Clone, Debug)]
pub(crate) struct CallExpression<'a> {
    pub(crate) function: Identifier<'a>,
    pub(crate) argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub(crate) struct UnaryExpression<'a> {
    pub(crate) operator: UnaryOperator,
    pub(crate) argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub(crate) struct BinaryExpression<'a> {
    pub(crate) operator: BinaryOperator,
    pub(crate) left: Box<Expression<'a>>,
    pub(crate) right: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub(crate) struct LogicalExpression<'a> {
    pub(crate) operator: LogicalOperator,
    pub(crate) left: Box<Expression<'a>>,
    pub(crate) right: Box<Expression<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BinaryOperator {
    StrictEqual,
    StrictNotEqual,
    LessThan,
    GreaterThan,
    LessThanEqual,
    GreaterThanEqual,
    Plus,
    Minus,
    Times,
    Over,
    Mod,
    In,
    PowerOf,
    Is,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LogicalOperator {
    Or,
    And,
}

impl LogicalOperator {
    pub(crate) fn short_circuit_on(&self, b: bool) -> bool {
        match self {
            Self::Or => b,
            Self::And => !b,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum UnaryOperator {
    Minus,
    Plus,
    Not,
}

#[derive(Clone, Debug)]
pub(crate) struct MemberExpression<'a> {
    pub(crate) object: Box<Expression<'a>>,
    pub(crate) property: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub(crate) enum Literal<'s> {
    Null,
    String(Cow<'s, str>),
    Number(Cow<'s, str>),
    Boolean(bool),
    Type(ValueType),
}
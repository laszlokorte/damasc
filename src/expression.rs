use std::borrow::Cow;

use crate::identifier::Identifier;
use crate::literal::Literal;

#[derive(Clone, Debug)]
pub enum Expression<'s> {
    Array(ArrayExpression<'s>),
    Binary(BinaryExpression<'s>),
    Identifier(Identifier<'s>),
    Literal(Literal<'s>),
    Logical(LogicalExpression<'s>),
    Member(MemberExpression<'s>),
    Object(ObjectExpression<'s>),
    Unary(UnaryExpression<'s>),
    Call(CallExpression<'s>),
    Template(StringTemplate<'s>),
}

impl std::fmt::Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Literal(l) => write!(f, "{l}"),
            _ => write!(f, "{self:?}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpressionSet<'s> {
    pub expressions: Vec<Expression<'s>>,
}

type ArrayExpression<'a> = Vec<ArrayItem<'a>>;

#[derive(Clone, Debug)]
pub enum ArrayItem<'a> {
    Single(Expression<'a>),
    Spread(Expression<'a>),
}

pub type ObjectExpression<'a> = Vec<ObjectProperty<'a>>;

#[derive(Clone, Debug)]
pub enum ObjectProperty<'a> {
    Single(Identifier<'a>),
    Property(Property<'a>),
    Spread(Expression<'a>),
}

#[derive(Clone, Debug)]
pub struct Property<'a> {
    pub key: PropertyKey<'a>,
    pub value: Expression<'a>,
}

#[derive(Clone, Debug)]
pub enum PropertyKey<'a> {
    Identifier(Identifier<'a>),
    Expression(Expression<'a>),
}

#[derive(Clone, Debug)]
pub struct CallExpression<'a> {
    pub function: Identifier<'a>,
    pub argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub struct StringTemplate<'a> {
    pub parts: Vec<StringTemplatePart<'a>>,
    pub suffix: Cow<'a, str>,
}

#[derive(Clone, Debug)]
pub struct StringTemplatePart<'a> {
    pub fixed_start: Cow<'a, str>,
    pub dynamic_end: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub struct UnaryExpression<'a> {
    pub operator: UnaryOperator,
    pub argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub struct BinaryExpression<'a> {
    pub operator: BinaryOperator,
    pub left: Box<Expression<'a>>,
    pub right: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
pub struct LogicalExpression<'a> {
    pub operator: LogicalOperator,
    pub left: Box<Expression<'a>>,
    pub right: Box<Expression<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryOperator {
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
    Cast,
}

#[derive(Clone, Copy, Debug)]
pub enum LogicalOperator {
    Or,
    And,
}

impl LogicalOperator {
    pub fn short_circuit_on(&self, b: bool) -> bool {
        match self {
            Self::Or => b,
            Self::And => !b,
        }
    }
}

#[derive(Clone, Debug)]
pub enum UnaryOperator {
    Minus,
    Plus,
    Not,
}

#[derive(Clone, Debug)]
pub struct MemberExpression<'a> {
    pub object: Box<Expression<'a>>,
    pub property: Box<Expression<'a>>,
}

use std::borrow::Cow;
use std::collections::VecDeque;

use crate::identifier::Identifier;
use crate::literal::Literal;
use gen_iter::gen_iter;

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
            Expression::Array(arr) => {
                write!(f, "[")?;
                for item in arr {
                    match item {
                        ArrayItem::Single(i) => write!(f, "{i},")?,
                        ArrayItem::Spread(i) => write!(f, "...({i}),")?,
                    }
                }
                write!(f, "[")
            },
            Expression::Binary(BinaryExpression {operator, left, right}) => {
                write!(f, "({left} {} {right})", match operator {
                    BinaryOperator::StrictEqual => "==",
                    BinaryOperator::StrictNotEqual => "!=",
                    BinaryOperator::LessThan => "<",
                    BinaryOperator::GreaterThan => ">",
                    BinaryOperator::LessThanEqual => "<=",
                    BinaryOperator::GreaterThanEqual => ">=",
                    BinaryOperator::Plus => "+",
                    BinaryOperator::Minus => "-",
                    BinaryOperator::Times => "*",
                    BinaryOperator::Over => "/",
                    BinaryOperator::Mod => "%",
                    BinaryOperator::In => "in",
                    BinaryOperator::PowerOf => "^",
                    BinaryOperator::Is => "is",
                    BinaryOperator::Cast => "cast",
                })
            },
            Expression::Identifier(id) => write!(f, "{id}"),
            Expression::Logical(LogicalExpression{left, right, operator}) => {
                write!(f, "({left} {} {right})", match operator {
                    LogicalOperator::Or => "||",
                    LogicalOperator::And => "&&",
                })
            },
            Expression::Member(MemberExpression{ object, property }) => {
                write!(f, "{object}[{property}]")
            },
            Expression::Object(props) => {
                write!(f, "{{")?;
                for prop in props {
                    match prop {
                        ObjectProperty::Single(id) => write!(f, "{id},")?,
                        ObjectProperty::Property(Property{ key, value }) => {
                            match key {
                                PropertyKey::Identifier(id) => write!(f, "{id}: {value},"),
                                PropertyKey::Expression(expr) => write!(f, "[{expr}]: {value},"),
                            }?;
                        },
                        ObjectProperty::Spread(expr) => write!(f, "...({expr}),")?,
                    }
                }
                write!(f, "}}")
            },
            Expression::Unary(UnaryExpression { operator, argument  }) => {
                write!(f, "({} {argument})", match operator {
                    UnaryOperator::Minus => "-",
                    UnaryOperator::Plus => "+",
                    UnaryOperator::Not => "!",
                })
            },
            Expression::Call(CallExpression { function, argument  }) => {
                write!(f, "{function}({argument})")
            },
            Expression::Template(StringTemplate { parts, suffix }) => {
                write!(f, "$`")?;
                for p in parts {
                    write!(f, "{}${{{}}}", p.fixed_start, p.dynamic_end)?;
                }
                write!(f, "{suffix}`")
            },
            
        }
    }
}

impl Expression<'_> {
    pub(crate) fn get_identifiers(&self) -> impl Iterator<Item = &Identifier> {
        gen_iter!(move {
            let mut expression_stack : VecDeque<&Expression> = VecDeque::new();
            
            expression_stack.push_front(self);

            while let Some(e) = expression_stack.pop_front() {
                match e {
                    Expression::Array(arr) => {
                        for item in arr {
                            match item {
                                ArrayItem::Single(s) => {
                                    expression_stack.push_front(s);
                                },
                                ArrayItem::Spread(s) => {
                                    expression_stack.push_front(s);
                                },
                            }
                        }
                    },
                    Expression::Binary(BinaryExpression {left, right,..}) => {
                        expression_stack.push_front(left);
                        expression_stack.push_front(right);
                    },
                    Expression::Identifier(id) => yield id,
                    Expression::Literal(_) => {},
                    Expression::Logical(LogicalExpression {left, right,..}) => {
                        expression_stack.push_front(left);
                        expression_stack.push_front(right);
                    },
                    Expression::Member(MemberExpression{ object, property }) => {
                        expression_stack.push_front(object);
                        expression_stack.push_front(property);
                    },
                    Expression::Object(props) => {
                        for p in props {
                            match p {
                                ObjectProperty::Single(s) => {
                                    yield s;
                                },
                                ObjectProperty::Property(Property{key, value}) => {
                                    expression_stack.push_front(value);

                                    match key {
                                        PropertyKey::Identifier(_id) => {},
                                        PropertyKey::Expression(expr) =>
                                        expression_stack.push_front(expr),
                                    };
                                },
                                ObjectProperty::Spread(s) => {
                                    expression_stack.push_front(s);
                                },
                            }
                        }
                    },
                    Expression::Unary(UnaryExpression{argument, ..}) => {
                        expression_stack.push_front(argument);
                    },
                    Expression::Call(CallExpression{argument,..}) => {
                        expression_stack.push_front(argument);

                    },
                    Expression::Template(StringTemplate{parts, ..}) => {
                        for p in parts {
                            expression_stack.push_front(&p.dynamic_end);
                        }
                    },
                }
            }
        })
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

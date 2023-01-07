use std::borrow::Cow;

use crate::value::ValueType;

#[derive(Clone, Debug)]
pub(crate) enum Literal<'s> {
    Null,
    String(Cow<'s, str>),
    Number(Cow<'s, str>),
    Boolean(bool),
    Type(ValueType),
}

impl<'a> std::fmt::Display for Literal<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Null => write!(f, "null"),
            Literal::String(s) => write!(f, "\"{s}\""),
            Literal::Number(n) => write!(f, "{n}"),
            Literal::Boolean(b) => write!(f, "{b}"),
            Literal::Type(t) => write!(f, "{t}"),
        }
    }
}
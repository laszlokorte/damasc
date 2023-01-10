#![feature(generators)]

pub mod assignment;
pub mod bag;
pub mod env;
pub mod expression;
pub mod identifier;
pub mod literal;
pub mod matcher;
pub mod parser;
pub mod pattern;
pub mod query;
pub mod repl;
pub mod statement;
pub mod typed_bag;
pub mod value;
pub mod wasm;

use expression::*;
use identifier::Identifier;
use literal::Literal;
use std::borrow::Cow;
use value::Value;

impl<'s, 'v> Value<'s, 'v> {
    pub fn to_expression(&self) -> Expression<'s> {
        match self {
            Value::Null => Expression::Literal(Literal::Null),
            Value::String(s) => Expression::Literal(Literal::String(s.clone())),
            Value::Integer(i) => Expression::Literal(Literal::Number(Cow::Owned(i.to_string()))),
            Value::Boolean(b) => Expression::Literal(Literal::Boolean(*b)),
            Value::Array(a) => Expression::Array(
                a.iter()
                    .map(|v| v.to_expression())
                    .map(ArrayItem::Single)
                    .collect(),
            ),
            Value::Object(o) => Expression::Object(
                o.iter()
                    .map(|(k, v)| {
                        ObjectProperty::Property(Property {
                            key: PropertyKey::Identifier(Identifier {
                                name: Cow::Owned(k.to_string()),
                            }),
                            value: v.to_expression(),
                        })
                    })
                    .collect(),
            ),
            Value::Type(t) => Expression::Literal(Literal::Type(*t)),
        }
    }
}

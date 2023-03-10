use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use crate::expression::PropertyKey;
use crate::literal::Literal;
use crate::pattern::*;
use crate::{env::Environment, identifier::Identifier, value::Value, value::ValueObjectMap};

#[derive(Debug)]
pub enum PatternFail {
    IdentifierConflict,
    ArrayMissmatch,
    ArrayLengthMismatch,
    TypeMismatch,
    ObjectMissmatch,
    ObjectLengthMismatch,
    ObjectKeyMismatch,
    EvalError,
    LiteralMismatch,
}

#[derive(Clone, Debug)]
pub struct Matcher<'i, 's, 'v, 'e> {
    pub outer_env: &'e Environment<'i, 's, 'v>,
    pub local_env: Environment<'i, 's, 'v>,
}

impl<'i, 's, 'v, 'e> Matcher<'i, 's, 'v, 'e> {
    pub fn into_env<'x>(self) -> Environment<'i, 's, 'v> {
        self.local_env
    }

    pub fn match_pattern<'x>(
        &'x mut self,
        pattern: &'x Pattern<'s>,
        value: &Value<'s, 'v>,
    ) -> Result<(), PatternFail> {
        match &pattern {
            Pattern::Discard => Ok(()),
            Pattern::Capture(name, pat) => self
                .match_pattern(pat, value)
                .and_then(|_| self.match_identifier(name, value)),
            Pattern::Identifier(name) => self.match_identifier(name, value),
            Pattern::TypedDiscard(t) => {
                if t == &value.get_type() {
                    Ok(())
                } else {
                    Err(PatternFail::TypeMismatch)
                }
            }
            Pattern::TypedIdentifier(name, t) => {
                if t != &value.get_type() {
                    return Err(PatternFail::TypeMismatch);
                }
                self.match_identifier(name, value)
            }
            Pattern::Object(pattern, rest) => {
                let Value::Object(o) = value else {
                    return Err(PatternFail::ObjectMissmatch);
                };
                self.match_object(pattern, rest, o)
            }
            Pattern::Array(items, rest) => {
                let Value::Array(a) = value else {
                    return Err(PatternFail::ArrayMissmatch);
                };
                self.match_array(items, rest, a)
            }
            Pattern::Literal(l) => self.match_literal(l, value),
        }
    }

    fn match_identifier<'x>(
        &'x mut self,
        name: &'x Identifier<'x>,
        value: &Value<'s, 'v>,
    ) -> Result<(), PatternFail> {
        let id = Identifier {
            name: Cow::Owned(name.name.to_string()),
        };

        match self.local_env.bindings.entry(id) {
            Entry::Occupied(entry) => {
                if value == entry.get() {
                    Ok(())
                } else {
                    Err(PatternFail::IdentifierConflict)
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(value.clone());
                Ok(())
            }
        }
    }

    fn match_object<'x>(
        &'x mut self,
        props: &[ObjectPropertyPattern<'s>],
        rest: &Rest<'s>,
        value: &ValueObjectMap<'s, 'v>,
    ) -> Result<(), PatternFail> {
        if let Rest::Exact = rest {
            if value.len() != props.len() {
                return Err(PatternFail::ObjectLengthMismatch);
            }
        }

        let mut keys = value.keys().collect::<BTreeSet<_>>();
        for prop in props {
            let (k, v) = match prop {
                ObjectPropertyPattern::Single(key) => {
                    (key.name.clone(), Pattern::Identifier(key.clone()))
                }
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Identifier(key),
                    value,
                }) => (key.name.clone(), value.clone()),
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Expression(exp),
                    value,
                }) => {
                    let Ok(Value::String(k)) = self.outer_env.eval_expr(exp) else {
                        return Err(PatternFail::EvalError);
                    };
                    (k.clone(), value.clone())
                }
            };

            if !keys.remove(&k) {
                return Err(PatternFail::ObjectKeyMismatch);
            }

            let Some(actual_value) = value.get(&k) else {
                return Err(PatternFail::ObjectKeyMismatch);
            };

            self.match_pattern(&v, actual_value.as_ref())?
        }

        if let Rest::Collect(rest_pattern) = rest {
            let remaining: BTreeMap<Cow<str>, Cow<Value>> = keys
                .iter()
                .map(|&k| (k.clone(), value.get(k).unwrap().clone()))
                .collect();
            self.match_pattern(rest_pattern, &Value::Object(remaining))
        } else {
            Ok(())
        }
    }

    fn match_array<'x>(
        &'x mut self,
        items: &[ArrayPatternItem<'s>],
        rest: &Rest<'s>,
        value: &Vec<Cow<'v, Value<'s, 'v>>>,
    ) -> Result<(), PatternFail> {
        if let Rest::Exact = rest {
            if value.len() != items.len() {
                return Err(PatternFail::ArrayLengthMismatch);
            }
        }

        if value.len() < items.len() {
            return Err(PatternFail::ArrayLengthMismatch);
        }

        for (ArrayPatternItem::Pattern(p), val) in std::iter::zip(items, value.iter()) {
            self.match_pattern(p, val.as_ref())?
        }

        if let Rest::Collect(rest_pattern) = rest {
            self.match_pattern(
                rest_pattern,
                &Value::Array(value.iter().skip(items.len()).cloned().collect()),
            )
        } else {
            Ok(())
        }
    }

    pub fn clear(&mut self) {
        self.local_env.bindings.clear();
    }

    fn match_literal(&self, literal: &Literal, value: &Value) -> Result<(), PatternFail> {
        let matches = match (literal, value) {
            (Literal::Null, Value::Null) => true,
            (Literal::String(a), Value::String(b)) => a == b,
            (Literal::Number(n), Value::Integer(i)) => {
                str::parse::<i64>(n).map(|p| &p == i).unwrap_or(false)
            }
            (Literal::Boolean(a), Value::Boolean(b)) => a == b,
            (Literal::Type(a), Value::Type(b)) => a == b,
            _ => false,
        };

        if matches {
            Ok(())
        } else {
            Err(PatternFail::LiteralMismatch)
        }
    }
    pub fn new<'x:'e>(env: &'x Environment<'i, 's, 'v>) -> Self {
        Self {
            outer_env: &env,
            local_env: Environment::new(),
        }
    }
}

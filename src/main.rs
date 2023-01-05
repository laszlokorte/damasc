#![feature(iter_array_chunks)]
#![feature(assert_matches)]
#![feature(map_try_insert)]
#![feature(let_chains)]

use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{alpha1, char, i64, multispace0};
use nom::combinator::{all_consuming, map, opt, recognize, value};
use nom::error::ParseError;
use nom::multi::{fold_many0, separated_list0};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated, tuple};
use nom::IResult;
use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use rustyline::error::ReadlineError;
use rustyline::Editor;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Value<'s, 'v> {
    Null,
    String(Cow<'s, str>),
    Integer(i64),
    Boolean(bool),
    Array(Vec<Cow<'v, Value<'s, 'v>>>),
    Object(BTreeMap<Cow<'s, str>, Cow<'v, Value<'s, 'v>>>),
    Type(ValueType),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum ValueType {
    Null,
    String,
    Integer,
    Boolean,
    Array,
    Object,
    Type,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl<'s, 'v> Value<'s, 'v> {
    pub(crate) fn to_expression(&self) -> Expression<'s> {
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

    fn get_type(&self) -> ValueType {
        match self {
            Value::Null => ValueType::Null,
            Value::String(_) => ValueType::String,
            Value::Integer(_) => ValueType::Integer,
            Value::Boolean(_) => ValueType::Boolean,
            Value::Array(_) => ValueType::Array,
            Value::Object(_) => ValueType::Object,
            Value::Type(_) => ValueType::Type,
        }
    }
}

impl<'s, 'v> std::fmt::Display for Value<'s, 'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Value::Null => write!(f, "null"),
            Value::String(s) => write!(f, "\"{s}\""),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Array(a) => {
                let _ = write!(f, "[");
                for v in a {
                    let _ = write!(f, "{v}, ",);
                }
                write!(f, "]")
            }
            Value::Object(o) => {
                let _ = write!(f, "{{");
                for (k, v) in o {
                    let _ = write!(f, "{k}: ",);
                    let _ = write!(f, "{v}, ",);
                }
                write!(f, "}}")
            }
            Value::Type(t) => write!(f, "{t}"),
        };
        write!(f, "")
    }
}

#[derive(Clone, Debug)]
enum Pattern<'s> {
    Discard,
    Identifier(Identifier<'s>),
    TypedDiscard(ValueType),
    TypedIdentifier(Identifier<'s>, ValueType),
    Object(ObjectPattern<'s>, Rest<'s>),
    Array(ArrayPattern<'s>, Rest<'s>),
}

impl<'a> std::fmt::Display for Pattern<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Pattern::Discard => write!(f, "_"),
            Pattern::TypedDiscard(t) => write!(f, "_ is {t}"),
            Pattern::Identifier(id) => write!(f, "{id}"),
            Pattern::TypedIdentifier(id, t) => write!(f, "{id} is {t}"),
            Pattern::Object(props, rest) => {
                let _ = write!(f, "{{");

                for prop in props {
                    let _ = match prop {
                        ObjectPropertyPattern::Single(p) => write!(f, "{p}"),
                        ObjectPropertyPattern::Match(PropertyPattern { key, value }) => {
                            let _ = match key {
                                PropertyKey::Identifier(id) => {
                                    write!(f, "{id}")
                                }
                                PropertyKey::Expression(e) => {
                                    write!(f, "{e}")
                                }
                            };

                            write!(f, ": {value}")
                        }
                    };
                    let _ = write!(f, ",");
                }

                match rest {
                    Rest::Exact => {}
                    Rest::Discard => {
                        let _ = write!(f, "...");
                    }
                    Rest::Collect(p) => {
                        let _ = write!(f, "...{p}");
                    }
                };

                write!(f, "}}")
            }
            Pattern::Array(items, rest) => {
                let _ = write!(f, "[");
                for ArrayPatternItem::Pattern(item) in items {
                    let _ = write!(f, "{item},");
                }

                match rest {
                    Rest::Exact => {}
                    Rest::Discard => {
                        let _ = write!(f, "...");
                    }
                    Rest::Collect(p) => {
                        let _ = write!(f, "...{p}");
                    }
                };
                write!(f, "]")
            }
        };
        write!(f, "")
    }
}

#[derive(Clone, Debug)]
enum Rest<'s> {
    Exact,
    Discard,
    Collect(Box<Pattern<'s>>),
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
struct Identifier<'a> {
    name: Cow<'a, str>,
}

impl std::fmt::Display for Identifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

type ObjectPattern<'a> = Vec<ObjectPropertyPattern<'a>>;
type ArrayPattern<'a> = Vec<ArrayPatternItem<'a>>;

#[derive(Clone, Debug)]
enum ArrayPatternItem<'a> {
    Pattern(Pattern<'a>),
    //Expression(Expression<'a>),
}

#[derive(Clone, Debug)]
enum ObjectPropertyPattern<'a> {
    Single(Identifier<'a>),
    Match(PropertyPattern<'a>),
}

#[derive(Clone, Debug)]
struct PropertyPattern<'a> {
    key: PropertyKey<'a>,
    value: Pattern<'a>,
}

#[derive(Clone, Debug)]
enum Expression<'s> {
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
enum ArrayItem<'a> {
    Single(Expression<'a>),
    Spread(Expression<'a>),
}

type ObjectExpression<'a> = Vec<ObjectProperty<'a>>;

#[derive(Clone, Debug)]
enum ObjectProperty<'a> {
    Single(Identifier<'a>),
    Property(Property<'a>),
    Spread(Expression<'a>),
}

#[derive(Clone, Debug)]
struct Property<'a> {
    key: PropertyKey<'a>,
    value: Expression<'a>,
}

#[derive(Clone, Debug)]
enum PropertyKey<'a> {
    Identifier(Identifier<'a>),
    Expression(Expression<'a>),
}

#[derive(Clone, Debug)]
struct CallExpression<'a> {
    function: Identifier<'a>,
    argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
struct UnaryExpression<'a> {
    operator: UnaryOperator,
    argument: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
struct BinaryExpression<'a> {
    operator: BinaryOperator,
    left: Box<Expression<'a>>,
    right: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
struct LogicalExpression<'a> {
    operator: LogicalOperator,
    left: Box<Expression<'a>>,
    right: Box<Expression<'a>>,
}

#[derive(Clone, Copy, Debug)]
enum BinaryOperator {
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
enum LogicalOperator {
    Or,
    And,
}
impl LogicalOperator {
    fn short_circuit_on(&self, b: bool) -> bool {
        match self {
            Self::Or => b,
            Self::And => !b,
        }
    }
}

#[derive(Clone, Debug)]
enum UnaryOperator {
    Minus,
    Plus,
    Not,
}

#[derive(Clone, Debug)]
struct MemberExpression<'a> {
    object: Box<Expression<'a>>,
    property: Box<Expression<'a>>,
}

#[derive(Clone, Debug)]
enum Literal<'s> {
    Null,
    String(Cow<'s, str>),
    Number(Cow<'s, str>),
    Boolean(bool),
    Type(ValueType),
}

#[derive(Clone)]
struct Environment<'i, 's, 'v> {
    bindings: BTreeMap<Identifier<'i>, Value<'s, 'v>>,
}

struct Matcher<'i, 's, 'v, 'e> {
    env: &'e Environment<'i, 's, 'v>,
    bindings: BTreeMap<Identifier<'i>, Value<'s, 'v>>,
}

#[derive(Debug)]
enum EvalError {
    KindError,
    TypeError,
    UnknownIdentifier,
    InvalidNumber,
    MathDivision,
    KeyNotDefined,
    OutOfBound,
    Overflow,
    UnknownFunction,
}

#[derive(Debug)]
enum PatternFail {
    IdentifierConflict,
    ArrayMissmatch,
    ArrayLengthMismatch,
    TypeMismatch,
    ObjectMissmatch,
    ObjectLengthMismatch,
    ObjectKeyMismatch,
    EvalError,
}

impl<'i, 's, 'v, 'e> Matcher<'i, 's, 'v, 'e> {
    fn match_pattern<'x>(
        &'x mut self,
        pattern: &'x Pattern<'s>,
        value: Value<'s, 'v>,
    ) -> Result<(), PatternFail> {
        match &pattern {
            Pattern::Discard => Ok(()),
            Pattern::Identifier(name) => self.match_identifier(name, &value),
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
                self.match_identifier(name, &value)
            }
            Pattern::Object(pattern, rest) => {
                let Value::Object(o) = value else {
                    return Err(PatternFail::ObjectMissmatch);
                };
                self.match_object(pattern, rest, &o)
            }
            Pattern::Array(items, rest) => {
                let Value::Array(a) = value else {
                    return Err(PatternFail::ArrayMissmatch);
                };
                self.match_array(items, rest, &a)
            }
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

        match self.bindings.entry(id) {
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
        value: &BTreeMap<Cow<'s, str>, Cow<'v, Value<'s, 'v>>>,
    ) -> Result<(), PatternFail> {
        if let Rest::Exact = rest && value.len() != props.len(){
            return Err(PatternFail::ObjectLengthMismatch);
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
                    let Ok(Value::String(k)) = self.env.eval_expr(exp) else {
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

            self.match_pattern(&v, actual_value.as_ref().clone())?
        }

        if let Rest::Collect(rest_pattern) = rest {
            let remaining: BTreeMap<Cow<str>, Cow<Value>> = keys
                .iter()
                .map(|&k| (k.clone(), value.get(k).unwrap().clone()))
                .collect();
            self.match_pattern(rest_pattern, Value::Object(remaining))
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
        if let Rest::Exact = rest && value.len() != items.len(){
            return Err(PatternFail::ArrayLengthMismatch);
        }

        if value.len() < items.len() {
            return Err(PatternFail::ArrayLengthMismatch);
        }

        for (item, val) in std::iter::zip(items, value.iter()) {
            let ArrayPatternItem::Pattern(p) = item;
            self.match_pattern(p, val.as_ref().clone())?
        }

        if let Rest::Collect(rest_pattern) = rest {
            self.match_pattern(
                rest_pattern,
                Value::Array(value.iter().skip(items.len()).cloned().collect()),
            )
        } else {
            Ok(())
        }
    }
}

impl<'i, 's, 'v> Environment<'i, 's, 'v> {
    fn clear(&mut self) {
        self.bindings.clear();
    }

    fn apply_matcher(&mut self, matcher: &mut Matcher<'i, 's, 'v, '_>) {
        self.bindings.append(&mut matcher.bindings);
    }

    fn eval_lit<'x>(&self, literal: &'x Literal<'x>) -> Result<Value<'s, 'v>, EvalError> {
        match literal {
            Literal::Null => Ok(Value::Null),
            Literal::String(s) => Ok(Value::<'s, 'v>::String(Cow::Owned(s.to_string()))),
            Literal::Number(s) => str::parse::<i64>(s)
                .map(Value::Integer)
                .map(Ok)
                .unwrap_or(Err(EvalError::InvalidNumber)),
            Literal::Boolean(b) => Ok(Value::Boolean(*b)),
            Literal::Type(t) => Ok(Value::Type(*t)),
        }
    }

    fn eval_expr<'x>(&self, expression: &'x Expression<'x>) -> Result<Value<'s, 'v>, EvalError> {
        match expression {
            Expression::Array(vec) => self.eval_array(vec),
            Expression::Binary(BinaryExpression {
                operator,
                left,
                right,
            }) => self.eval_expr(left).and_then(|l| {
                self.eval_expr(right)
                    .and_then(|r| self.eval_binary(operator, &l, &r))
            }),
            Expression::Identifier(id) => self.eval_identifier(id),
            Expression::Literal(l) => self.eval_lit(l),
            Expression::Logical(LogicalExpression {
                operator,
                left,
                right,
            }) => self.eval_logic(operator, left, right),
            Expression::Member(MemberExpression {
                object, property, ..
            }) => self.eval_expr(object).and_then(move |obj| {
                self.eval_expr(property)
                    .and_then(move |prop| self.eval_member(&obj, &prop))
            }),
            Expression::Object(props) => self.eval_object(props),
            Expression::Unary(UnaryExpression {
                operator, argument, ..
            }) => self
                .eval_expr(argument)
                .and_then(|v| self.eval_unary(operator, &v)),
            Expression::Call(CallExpression { function, argument }) => {
                self.eval_call(function, &self.eval_expr(argument)?)
            }
        }
    }

    fn eval_binary<'x>(
        &self,
        op: &BinaryOperator,
        left: &Value<'s, 'x>,
        right: &Value<'s, 'x>,
    ) -> Result<Value<'s, 'v>, EvalError> {
        match op {
            BinaryOperator::StrictEqual => Ok(Value::Boolean(left == right)),
            BinaryOperator::StrictNotEqual => Ok(Value::Boolean(left != right)),
            BinaryOperator::LessThan => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l < r))
            }
            BinaryOperator::GreaterThan => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l > r))
            }
            BinaryOperator::LessThanEqual => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l <= r))
            }
            BinaryOperator::GreaterThanEqual => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l >= r))
            }
            BinaryOperator::Plus => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_add(*r)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::Minus => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_sub(*r)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::Times => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_mul(*r)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::Over => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                if *r == 0 {
                    return Err(EvalError::MathDivision);
                }
                l.checked_div(*r)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::Mod => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_rem(*r)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::In => {
                let Value::String(s) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Object(o) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(o.contains_key(s)))
            }
            BinaryOperator::PowerOf => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_pow(*r as u32)
                    .map(Value::Integer)
                    .map(Ok)
                    .unwrap_or(Err(EvalError::Overflow))
            }
            BinaryOperator::Is => {
                let Value::Type(specified_type) = right else {
                    return Err(EvalError::KindError);
                };
                let actual_type = left.get_type();

                Ok(Value::Boolean(actual_type == *specified_type))
            }
        }
    }

    fn eval_unary(&self, op: &UnaryOperator, arg: &Value) -> Result<Value<'s, 'v>, EvalError> {
        match op {
            UnaryOperator::Minus => {
                let Value::Integer(v) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(-v))
            }
            UnaryOperator::Plus => {
                let Value::Integer(v) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(*v))
            }
            UnaryOperator::Not => {
                let Value::Boolean(b) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(!b))
            }
        }
    }

    fn eval_object<'x>(&self, props: &'x ObjectExpression<'x>) -> Result<Value<'s, 'v>, EvalError> {
        let mut kv_map = BTreeMap::new();

        for prop in props {
            match prop {
                ObjectProperty::Single(id @ Identifier { name }) => {
                    let keyval = Cow::Owned(name.to_string());
                    let valval = self.eval_identifier(id)?;

                    kv_map.insert(keyval, Cow::Owned(valval.to_owned()));
                }
                ObjectProperty::Property(Property {
                    key,
                    value: value_expr,
                }) => {
                    let keyval = match key {
                        PropertyKey::Identifier(Identifier { name }) => {
                            Cow::Owned(name.to_string())
                        }
                        PropertyKey::Expression(e) => {
                            let val = self.eval_expr(e)?;
                            let Value::String(s) = val else {
                                return Err(EvalError::TypeError);
                            };
                            s
                        }
                    };
                    let valval = self.eval_expr(value_expr)?;
                    kv_map.insert(keyval, Cow::Owned(valval.to_owned()));
                }
                ObjectProperty::Spread(expr) => {
                    let to_spread = self.eval_expr(expr)?;
                    let Value::Object(map) = to_spread else {
                        return Err(EvalError::TypeError)
                    };
                    for (k, v) in map {
                        kv_map.insert(k, v);
                    }
                }
            }
        }

        Ok(Value::<'s, 'v>::Object(kv_map))
    }

    fn eval_array<'x>(&self, vec: &'x [ArrayItem<'x>]) -> Result<Value<'s, 'v>, EvalError> {
        let mut result = vec![];

        for item in vec {
            match item {
                ArrayItem::Single(exp) => {
                    let v = self.eval_expr(exp)?;

                    result.push(Cow::Owned(v));
                }
                ArrayItem::Spread(exp) => {
                    let v = self.eval_expr(exp)?;
                    let Value::Array(mut multiples) = v else {
                        return Err(EvalError::TypeError);
                    };

                    result.append(&mut multiples);
                }
            }
        }

        Ok(Value::Array(result))
    }

    fn eval_logic<'x>(
        &self,
        operator: &LogicalOperator,
        left: &'x Expression<'x>,
        right: &'x Expression<'x>,
    ) -> Result<Value<'s, 'v>, EvalError> {
        let left_value = self.eval_expr(left)?;
        let Value::Boolean(left_bool) = left_value else {
            return Err(EvalError::TypeError);
        };
        if operator.short_circuit_on(left_bool) {
            return Ok(Value::Boolean(left_bool));
        }
        let right_value = self.eval_expr(right)?;
        let Value::Boolean(right_bool) = right_value else {
            return Err(EvalError::TypeError);
        };
        return Ok(Value::Boolean(right_bool));
    }

    fn eval_member<'x: 'v>(
        &self,
        obj: &Value<'s, 'x>,
        prop: &Value<'s, 'x>,
    ) -> Result<Value<'s, 'x>, EvalError> {
        match obj {
            Value::Object(o) => {
                let Value::String(p) = prop else {
                    return Err(EvalError::TypeError);
                };

                let Some(val) = o.get(p).map(|v|v.clone().into_owned()) else {
                    return Err(EvalError::KeyNotDefined);
                };

                Ok(val)
            }
            Value::Array(a) => {
                let Value::Integer(i) = prop else {
                    return Err(EvalError::TypeError);
                };
                let index = if *i < 0 {
                    a.len() - i.unsigned_abs() as usize
                } else {
                    *i as usize
                };

                let Some(val) = a.get(index).map(|v|v.clone().into_owned()) else {
                    return Err(EvalError::OutOfBound);
                };

                Ok(val)
            }
            Value::String(s) => {
                let Value::Integer(i) = prop else {
                    return Err(EvalError::TypeError);
                };
                let index = if *i < 0 {
                    s.len() - i.unsigned_abs() as usize
                } else {
                    *i as usize
                };

                let Some(val) = s.chars().nth(index).map(|v|v.clone().to_string()) else {
                    return Err(EvalError::OutOfBound);
                };

                Ok(Value::String(Cow::Owned(val)))
            }
            _ => Err(EvalError::TypeError),
        }
    }

    fn eval_identifier(&self, id: &Identifier) -> Result<Value<'s, 'v>, EvalError> {
        let Some(val) = self.bindings.get(id) else {
            return Err(EvalError::UnknownIdentifier);
        };

        Ok(val.clone())
    }

    fn eval_call(
        &self,
        function: &Identifier,
        argument: &Value<'s, 'v>,
    ) -> Result<Value<'s, 'v>, EvalError> {
        Ok(match function.name.as_ref() {
            "length" => Value::Integer(match argument {
                Value::String(s) => s.len() as i64,
                Value::Array(a) => a.len() as i64,
                Value::Object(o) => o.len() as i64,
                _ => return Err(EvalError::TypeError),
            }),
            "keys" => Value::Array(match argument {
                Value::Object(o) => o
                    .keys()
                    .map(|k| Cow::Owned(Value::String(Cow::Owned(k.to_string()))))
                    .collect(),
                _ => return Err(EvalError::TypeError),
            }),
            "values" => Value::Array(match argument {
                Value::Object(o) => o.values().cloned().collect(),
                _ => return Err(EvalError::TypeError),
            }),
            "type" => Value::Type(argument.get_type()),
            _ => return Err(EvalError::UnknownFunction),
        })
    }
}

fn array_item_expression<'v>(input: &str) -> IResult<&str, ArrayItem<'v>> {
    alt((
        map(preceded(ws(tag("...")), expression), ArrayItem::Spread),
        map(expression, ArrayItem::Single),
    ))(input)
}

fn ws<'a, F, O, E: ParseError<&'a str>>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn expression_call<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            identifier,
            delimited(ws(tag("(")), expression, ws(tag(")"))),
        ),
        |(function, arg)| {
            Expression::Call(CallExpression {
                function,
                argument: Box::new(arg),
            })
        },
    )(input)
}

fn expression_array<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(
        ws(tag("[")),
        terminated(
            map(
                separated_list0(ws(tag(",")), array_item_expression),
                Expression::Array,
            ),
            opt(ws(tag(","))),
        ),
        ws(tag("]")),
    )(input)
}

fn object_prop_expression<'v>(input: &str) -> IResult<&str, ObjectProperty<'v>> {
    alt((
        map(
            separated_pair(
                delimited(ws(tag("[")), expression, ws(tag("]"))),
                ws(tag(":")),
                expression,
            ),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Expression(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(identifier, ws(tag(":")), expression),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Identifier(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(literal_string_raw, ws(tag(":")), expression),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Identifier(Identifier { name: prop }),
                    value,
                })
            },
        ),
        map(preceded(ws(tag("...")), expression), ObjectProperty::Spread),
        map(identifier, ObjectProperty::Single),
    ))(input)
}

fn expression_object<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(
        ws(tag("{")),
        terminated(
            map(
                separated_list0(ws(ws(tag(","))), object_prop_expression),
                Expression::Object,
            ),
            opt(ws(tag(","))),
        ),
        ws(tag("}")),
    )(input)
}

fn expression_literal<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((
        expression_object,
        expression_array,
        expression_call,
        expression_atom,
    ))(input)
}

fn expression_atom<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        alt((
            literal_null,
            literal_string,
            literal_bool,
            literal_number,
            literal_type,
        )),
        Expression::Literal,
    )(input)
}

fn expression_identifier<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(identifier, Expression::Identifier)(input)
}

fn literal_null<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    value(Literal::Null, tag("null"))(input)
}

fn literal_string_raw<'v>(input: &str) -> IResult<&str, Cow<'v, str>> {
    map(
        delimited(tag("\""), take_until("\""), tag("\"")),
        |s: &str| Cow::Owned(s.to_string()),
    )(input)
}

fn literal_string<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(literal_string_raw, Literal::String)(input)
}

fn literal_bool<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    alt((
        value(Literal::Boolean(true), tag("true")),
        value(Literal::Boolean(false), tag("false")),
    ))(input)
}

fn literal_number<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(recognize(i64), |s: &str| {
        Literal::Number(Cow::Owned(s.to_owned()))
    })(input)
}

fn identifier<'v>(input: &str) -> IResult<&str, Identifier<'v>> {
    map(alpha1, |name: &str| Identifier {
        name: Cow::Owned(name.to_string()),
    })(input)
}

fn expression_logic_additive<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_logic_multiplicative(input)?;

    fold_many0(
        pair(
            ws(alt((value(LogicalOperator::Or, tag("||")),))),
            expression_logic_multiplicative,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_logic_multiplicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_type_predicate(input)?;

    fold_many0(
        pair(
            ws(alt((value(LogicalOperator::And, tag("&&")),))),
            expression_type_predicate,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn literal_type_raw(input: &str) -> IResult<&str, ValueType> {
    alt((
        value(ValueType::Type, tag("Type")),
        value(ValueType::Null, tag("Null")),
        value(ValueType::Boolean, tag("Boolean")),
        value(ValueType::Integer, tag("Integer")),
        value(ValueType::Array, tag("Array")),
        value(ValueType::Object, tag("Object")),
        value(ValueType::String, tag("String")),
    ))(input)
}

fn literal_type<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(literal_type_raw, Literal::Type)(input)
}

fn expression_type_predicate<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_predicative(input)?;

    let Ok((input, t)) = preceded(ws(tag("is")), expression_numeric_predicative)(input) else {
        return Ok((input, init));
    };

    Ok((
        input,
        Expression::Binary(BinaryExpression {
            operator: BinaryOperator::Is,
            left: Box::new(init),
            right: Box::new(t),
        }),
    ))
}

fn expression_numeric_predicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_additive(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::GreaterThanEqual, tag(">=")),
                value(BinaryOperator::LessThanEqual, tag("<=")),
                value(BinaryOperator::LessThan, char('<')),
                value(BinaryOperator::GreaterThan, char('>')),
                value(BinaryOperator::StrictEqual, tag("==")),
                value(BinaryOperator::StrictNotEqual, tag("!=")),
                value(BinaryOperator::In, tag("in")),
            ))),
            expression_numeric_additive,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_additive<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_multiplicative(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::Plus, char('+')),
                value(BinaryOperator::Minus, char('-')),
            ))),
            expression_numeric_multiplicative,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_multiplicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_exponential(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::Times, char('*')),
                value(BinaryOperator::Over, char('/')),
                value(BinaryOperator::Mod, char('%')),
            ))),
            expression_numeric_exponential,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_exponential<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_indexed(input)?;

    fold_many0(
        pair(
            ws(alt((value(BinaryOperator::PowerOf, char('^')),))),
            expression_indexed,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_indexed<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_member(input)?;

    fold_many0(
        delimited(ws(tag("[")), expression, ws(tag("]"))),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression {
                object: Box::new(acc),
                property: Box::new(ident),
            })
        },
    )(input)
}

fn expression_member<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_primary(input)?;

    fold_many0(
        alt((preceded(ws(char('.')), identifier),)),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression {
                object: Box::new(acc),
                property: Box::new(Expression::Literal(Literal::String(ident.name))),
            })
        },
    )(input)
}

fn expression_primary<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((
        expression_with_paren,
        expression_literal,
        expression_identifier,
        expression_unary,
    ))(input)
}

fn expression_with_paren<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(tag("("), expression, tag(")"))(input)
}

fn expression_unary<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((expression_unary_logic, expression_unary_numeric))(input)
}

fn expression_unary_logic<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            ws(alt((value(UnaryOperator::Not, tag("!")),))),
            expression_primary,
        ),
        |(operator, argument)| {
            Expression::Unary(UnaryExpression {
                operator,
                argument: Box::new(argument),
            })
        },
    )(input)
}

fn expression_unary_numeric<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            ws(alt((
                value(UnaryOperator::Minus, tag("-")),
                value(UnaryOperator::Plus, tag("+")),
            ))),
            alt((expression_indexed,)),
        ),
        |(operator, argument)| {
            Expression::Unary(UnaryExpression {
                operator,
                argument: Box::new(argument),
            })
        },
    )(input)
}

fn expression<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((expression_logic_additive,))(input)
}

fn full_expression<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    all_consuming(expression)(input)
}

fn full_pattern<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    all_consuming(pattern)(input)
}

fn pattern_discard<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    value(Pattern::Discard, tag("_"))(input)
}

fn pattern_typed_discard<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        preceded(ws(tag("_ is ")), literal_type_raw),
        Pattern::TypedDiscard,
    )(input)
}

fn pattern_identifier<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(identifier, Pattern::Identifier)(input)
}

fn pattern_typed_identifier<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        separated_pair(identifier, tag(" is "), literal_type_raw),
        |(i, t)| Pattern::TypedIdentifier(i, t),
    )(input)
}

fn object_prop_pattern<'v>(input: &str) -> IResult<&str, ObjectPropertyPattern<'v>> {
    alt((
        map(
            separated_pair(
                delimited(ws(tag("[")), expression, ws(tag("]"))),
                ws(tag(":")),
                pattern,
            ),
            |(prop, value)| {
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Expression(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(identifier, ws(tag(":")), pattern),
            |(prop, value)| {
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Identifier(prop),
                    value,
                })
            },
        ),
        map(identifier, ObjectPropertyPattern::Single),
    ))(input)
}

fn pattern_object<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    delimited(
        ws(tag("{")),
        alt((
            map(pattern_rest, |r| Pattern::Object(vec![], r)),
            map(
                tuple((
                    separated_list0(ws(ws(tag(","))), object_prop_pattern),
                    opt(preceded(ws(tag(",")), pattern_rest)),
                )),
                |(props, rest)| Pattern::Object(props, rest.unwrap_or(Rest::Exact)),
            ),
        )),
        ws(tag("}")),
    )(input)
}

fn pattern_rest<'v>(input: &str) -> IResult<&str, Rest<'v>> {
    alt((
        map(preceded(ws(tag("...")), pattern), |r| {
            Rest::Collect(Box::new(r))
        }),
        value(Rest::Discard, ws(tag("..."))),
    ))(input)
}

fn pattern_array<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    delimited(
        ws(tag("[")),
        alt((
            map(pattern_rest, |r| Pattern::Array(vec![], r)),
            map(
                tuple((
                    separated_list0(ws(tag(",")), map(pattern, ArrayPatternItem::Pattern)),
                    opt(preceded(ws(tag(",")), pattern_rest)),
                )),
                |(items, rest)| Pattern::Array(items, rest.unwrap_or(Rest::Exact)),
            ),
        )),
        ws(tag("]")),
    )(input)
}

fn pattern<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    alt((
        pattern_array,
        pattern_typed_discard,
        pattern_typed_identifier,
        pattern_discard,
        pattern_identifier,
        pattern_object,
    ))(input)
}

#[derive(Clone)]
enum Statement<'a, 'b> {
    Clear,
    Inspect(Expression<'b>),
    Format(Expression<'b>),
    Eval(Expression<'b>),
    Literal(Expression<'b>),
    Pattern(Pattern<'b>),
    Assign(Pattern<'a>, Expression<'b>),
    Match(Pattern<'a>, Expression<'b>),
}

fn assignment<'v, 'w>(input: &str) -> IResult<&str, Statement<'v, 'w>> {
    map(
        preceded(
            ws(tag("let ")),
            separated_pair(pattern, ws(tag("=")), full_expression),
        ),
        |(pat, expr)| Statement::Assign(pat, expr),
    )(input)
}

fn try_match<'v, 'w>(input: &str) -> IResult<&str, Statement<'v, 'w>> {
    map(
        separated_pair(pattern, ws(tag("=")), full_expression),
        |(pat, expr)| Statement::Match(pat, expr),
    )(input)
}

fn statement<'a, 'b>(input: &str) -> IResult<&str, Statement<'a, 'b>> {
    alt((
        value(Statement::Clear, tag(".clear")),
        map(
            preceded(tag(".inspect "), full_expression),
            Statement::Inspect,
        ),
        map(
            preceded(tag(".format "), full_expression),
            Statement::Format,
        ),
        map(preceded(tag(".pattern "), full_pattern), Statement::Pattern),
        map(
            preceded(tag(".literal "), full_expression),
            Statement::Literal,
        ),
        assignment,
        try_match,
        map(full_expression, Statement::Eval),
    ))(input)
}

fn main() -> rustyline::Result<()> {
    let mut env = Environment {
        bindings: BTreeMap::new(),
    };

    let mut rl = Editor::<()>::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                let input = line.as_str();

                let stmt = match statement(input) {
                    Ok((_, s)) => s,
                    Err(e) => {
                        println!("read error: {e}");
                        continue;
                    }
                };

                match stmt {
                    Statement::Clear => {
                        env.clear();
                    }

                    Statement::Inspect(ex) => {
                        dbg!(ex);
                    }
                    Statement::Format(ex) => {
                        println!("{ex:?}");
                    }
                    Statement::Eval(ex) => {
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            }
                        };

                        println!("{result}");
                    }
                    Statement::Assign(pattern, ex) => {
                        let mut matcher = Matcher {
                            env: &env.clone(),
                            bindings: BTreeMap::new(),
                        };
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            }
                        };

                        match matcher.match_pattern(&pattern, result.clone()) {
                            Ok(_) => {
                                for (id, v) in &matcher.bindings {
                                    println!("let {id} = {v}");
                                }
                                env.apply_matcher(&mut matcher);
                            }
                            Err(e) => {
                                println!("NO: {e:?}")
                            }
                        }
                    }
                    Statement::Match(pattern, ex) => {
                        let mut matcher = Matcher {
                            env: &env.clone(),
                            bindings: BTreeMap::new(),
                        };
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            }
                        };

                        match matcher.match_pattern(&pattern, result.clone()) {
                            Ok(_) => {
                                println!("YES:");

                                for (id, v) in &matcher.bindings {
                                    println!("{id} = {v}");
                                }
                            }
                            Err(e) => {
                                println!("NO: {e:?}")
                            }
                        }
                    }
                    Statement::Literal(ex) => {
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r.to_expression(),
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            }
                        };

                        println!("{result}");
                    }
                    Statement::Pattern(pattern) => {
                        dbg!(&pattern);
                    }
                };
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        }
    }
    rl.save_history("history.txt")
}

#[cfg(test)]
mod test {
    use std::assert_matches::assert_matches;

    use super::*;

    fn full_matching(input: &str) -> IResult<&str, (Pattern, Expression)> {
        all_consuming(separated_pair(pattern, ws(tag("=")), expression))(input)
    }

    #[test]
    fn test_expressions() {
        let tests = include_str!("test_expressions.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for [expr, result, sep] in tests.into_iter().array_chunks() {
            assert_eq!("---", sep);
            let parsed = full_expression(expr);
            let value = full_expression(result);
            assert!(parsed.is_ok());

            assert!(value.is_ok());

            let evaled = env.eval_expr(&parsed.unwrap().1);
            let valued_evaled = env.eval_expr(&value.unwrap().1);

            dbg!(&expr);
            assert!(evaled.is_ok());
            assert!(valued_evaled.is_ok());

            assert_eq!(evaled.unwrap(), valued_evaled.unwrap());
        }
    }

    #[test]
    fn test_patterns() {
        let tests = include_str!("test_patterns.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for case in tests {
            let mut matcher = Matcher {
                env: &env,
                bindings: BTreeMap::new(),
            };

            let Ok((_, (pattern, expr))) = full_matching(case) else {
                dbg!(case);
                unreachable!();
            };

            let Ok(value) = env.eval_expr(&expr) else {
                unreachable!();
            };
            dbg!(case);

            assert_matches!(matcher.match_pattern(&pattern, value), Ok(_));
        }
    }

    #[test]
    fn test_negative_patterns() {
        let tests = include_str!("test_negative_patterns.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for case in tests {
            let mut matcher = Matcher {
                env: &env,
                bindings: BTreeMap::new(),
            };
            let Ok((_, (pattern, expr))) = full_matching(case) else {
                dbg!(case);
                unreachable!();
            };

            let Ok(value) = env.eval_expr(&expr) else {
                unreachable!();
            };
            dbg!(case);

            assert_matches!(matcher.match_pattern(&pattern, value), Err(_));
        }
    }
}

#![feature(iter_array_chunks)]
#![feature(assert_matches)]
#![feature(map_try_insert)]
#![feature(let_chains)]

use std::borrow::{Cow, Borrow};
use std::collections::btree_map::{Entry, OccupiedEntry};
use std::collections::{BTreeMap, BTreeSet};
use nom::IResult;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{multispace0, i32, alpha1, char};
use nom::combinator::{map, value, recognize, all_consuming, opt};
use nom::error::ParseError;
use nom::multi::{separated_list0, fold_many0};
use nom::sequence::{delimited, separated_pair, preceded, pair, terminated, tuple};

use rustyline::error::ReadlineError;
use rustyline::{Editor};

#[derive(Debug, Clone,Eq,PartialEq,Ord,PartialOrd)]
enum Value<'a> {
    Null,
    String(Cow<'a, str>),
    Integer(i32),
    Boolean(bool),
    Array(Vec<Cow<'a, Value<'a>>>),
    Object(BTreeMap<Cow<'a, str>, Cow<'a, Value<'a>>>),
    Type(ValueType),
}

#[derive(Debug, Copy, Clone,Eq,PartialEq,Ord,PartialOrd)]
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

impl <'a> Value<'a> {
    fn new_null() -> Self {
        Self::Null
    }

    fn new_string(v: String) -> Self {
        Self::String(Cow::Owned(v))
    }

    fn new_int(v: i32) -> Self {
        Self::Integer(v)
    }

    fn new_bool(v: bool) -> Self {
        Self::Boolean(v)
    }

    fn new_array(v: &'a [Value<'a>]) -> Self {
        Self::Array(v.iter().map(Cow::Borrowed).collect())
    }

    fn new_object<'x:'a>(v: &'x [(String, Value<'a>)]) -> Self {
        Self::Object(v.iter().map(|(k,v)| (Cow::Owned(k.clone()), Cow::Borrowed(v))).collect())
    }

    fn to_expression(&self) -> Expression {
        match self {
            Value::Null => Expression::Literal(Literal::Null),
            Value::String(s) => Expression::Literal(Literal::String(s.clone())),
            Value::Integer(i) => Expression::Literal(Literal::Number(Cow::Owned(i.to_string()))),
            Value::Boolean(b) => Expression::Literal(Literal::Boolean(*b)),
            Value::Array(a) => {
                Expression::Array(a.iter().map(|v| v.to_expression()).map(ArrayItem::Single).collect())
            },
            Value::Object(o) => {
                Expression::Object(o.iter().map(|(k,v)| ObjectProperty::Property(Property { key: PropertyKey::Identifier(Identifier{name: Cow::Borrowed(k)}), value: v.to_expression() })).collect())
            },
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

impl <'a> std::fmt::Display for Value<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Value::Null => write!(f,"null"),
            Value::String(s) => write!(f,"\"{s}\""),
            Value::Integer(i) => write!(f,"{i}"),
            Value::Boolean(b) => write!(f,"{b}"),
            Value::Array(a) => {
                let _ = write!(f,"[");
                for v in a {
                    let _ = write!(f,"{v}, ",);
                }
                write!(f,"]")
            },
            Value::Object(o) => {
                let _ = write!(f,"{{");
                for (k,v) in o {
                    let _ = write!(f,"{k}: ",);
                    let _ = write!(f,"{v}, ",);
                }
                write!(f,"}}")
            },
            Value::Type(t) => write!(f,"{t}"),
        };
        write!(f,"")
    }
}

#[derive(Clone, Debug)]
enum Pattern<'a> {
    Discard,
    Identifier(Identifier<'a>),
    Object(ObjectPattern<'a>, Rest<'a>),
    Array(ArrayPattern<'a>, Rest<'a>),
}


impl <'a> std::fmt::Display for Pattern<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Pattern::Discard => write!(f,"_"),
            Pattern::Identifier(id) => write!(f,"{id}"),
            Pattern::Object(props, rest) => {
                
                write!(f,"{{");

                for prop in props {
                    let _ = match prop {
                        ObjectPropertyPattern::Single(p) => 
                        write!(f,"{p}"),
                        ObjectPropertyPattern::Match(PropertyPattern{
                            key,
                            value,
                        }) => {
                            match key {
                                PropertyKey::Identifier(id) => {
                                    write!(f,"{id}")
                                },
                                PropertyKey::Expression(e) => {
                                    write!(f,"?")
                                },
                            }
                        }
                    };
                }
                
                match rest {
                    Rest::Exact => {},
                    Rest::Discard => {let _ = write!(f,"...");},
                    Rest::Collect(p) => {
                       let _ =  write!(f,"...{p}");
                    },
                };

                write!(f,"}}")
            },
            Pattern::Array(_, _) => todo!(),
        };
        write!(f,"")
    }
}

#[derive(Clone, Debug)]
enum Rest<'a> {
    Exact,
    Discard,
    Collect(Box<Pattern<'a>>),
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
enum Expression<'a> {
    Array(ArrayExpression<'a>),
    Binary(BinaryExpression<'a>),
    Identifier(Identifier<'a>),
    Literal(Literal<'a>),
    Logical(LogicalExpression<'a>),
    Member(MemberExpression<'a>),
    Object(ObjectExpression<'a>),
    Unary(UnaryExpression<'a>),
    Call(CallExpression<'a>)
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

#[derive(Clone,Copy, Debug)]
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

#[derive(Clone,Copy, Debug)]
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
enum Literal<'a> {
    Null,
    String(Cow<'a, str>),
    Number(Cow<'a, str>),
    Boolean(bool),
    Type(ValueType),
}



#[derive(Clone)]
struct Environment<'i, 'e> {
    bindings: BTreeMap<Identifier<'i>, Value<'e>>
}

struct Matcher<'e, 'i, 'm> {
    env: &'e Environment<'i, 'm>,
    bindings: BTreeMap<Identifier<'i>, Value<'m>>
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

impl <'e, 'i,'m> Matcher<'e, 'i,'m> {
    fn match_pattern(&mut self, pattern: &Pattern<'m>, value: Value<'m>) -> bool {
        match (&pattern, &value) {
            (Pattern::Discard, _) => true,
            (Pattern::Identifier(name), _) => self.match_identifier(name, &value),
            (Pattern::Object(pattern, rest), Value::Object(o)) => self.match_object(pattern, rest, o),
            (Pattern::Array(items, rest), Value::Array(a)) => self.match_array(items, rest, a),
            _ => false,
        }
    }

    fn match_identifier<'x>(&mut self, name: &'x Identifier<'x>, value: &Value<'m>) -> bool {
        let id = Identifier { name: Cow::Owned(name.name.to_string()) };

        match self.bindings.entry(id) {
            Entry::Occupied(entry) => value == entry.get(),
            Entry::Vacant(entry) => {entry.insert(value.clone()); true},
        }
    }

    fn match_object(&mut self, props: &[ObjectPropertyPattern<'m>], rest: &Rest<'m>, value: &BTreeMap<Cow<'m, str>, Cow<'m, Value<'m>>>) -> bool {
        if let Rest::Exact = rest && value.len() != props.len(){
            return false;
        }

        let mut keys = value.keys().collect::<BTreeSet<_>>();
        for prop in props {
            let (k, v) = match prop {
                ObjectPropertyPattern::Single(key) => (key.name.clone(), Pattern::Identifier(key.clone())),
                ObjectPropertyPattern::Match(PropertyPattern{key: PropertyKey::Identifier(key), value}) => (key.name.clone(), value.clone()),
                ObjectPropertyPattern::Match(PropertyPattern{key: PropertyKey::Expression(exp), value}) => {
                    let Ok(Value::String(k)) = self.env.eval_expr(exp) else {
                        return false;
                    };
                    (k.clone(), value.clone())
                },
            };

            if !keys.remove(&k) {
                return false;
            }

            let Some(actual_value) = value.get(&k) else {
                return false;
            };

            if !self.match_pattern(&v, actual_value.as_ref().clone()) {
                return false;
            }
        }

        let rest_matches = if let Rest::Collect(rest_pattern) = rest {
            let remaining : BTreeMap<Cow<str>, Cow<Value>> = keys.iter().map(|&k| (k.clone(), value.get(k).unwrap().clone())).collect();
            self.match_pattern(&rest_pattern, Value::Object(remaining))
        } else {
            true
        };

        rest_matches
    }

    fn match_array(&mut self, items: &[ArrayPatternItem<'m>], rest: &Rest<'m>, value: &Vec<Cow<'m, Value<'m>>>) -> bool {
        if let Rest::Exact = rest && value.len() != items.len(){
            return false;
        }

        if value.len() < items.len() {
            return false;
        }

        for (item, val) in std::iter::zip(items, value.iter()) {
            let ArrayPatternItem::Pattern(p) = item;
            if !self.match_pattern(&p, val.as_ref().clone()) {
                return false;
            }
        }
        
        let rest_matches = if let Rest::Collect(rest_pattern) = rest {
            self.match_pattern(rest_pattern, Value::Array(value.iter().skip(items.len()).cloned().collect()))
        } else {
            true
        };

        rest_matches
    }

    fn clear(&mut self) {
        self.bindings.clear();
    }
}

impl <'i, 'e> Environment<'i, 'e> {

    

    fn eval_lit<'x>(&self, literal: &'x Literal<'x>) -> Result<Value<'e>, EvalError> {
        match literal {
            Literal::Null => Ok(Value::Null),
            Literal::String(s) => Ok(Value::<'e>::String(Cow::Owned(s.to_string()))),
            Literal::Number(s) => str::parse::<i32>(s).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::InvalidNumber)),
            Literal::Boolean(b) => Ok(Value::Boolean(*b)),
            Literal::Type(t) => Ok(Value::Type(*t)),
        }
    }
    
    fn eval_expr<'x>(&self, expression: &'x Expression<'x>) -> Result<Value<'e>, EvalError> {
        match expression {
            Expression::Array(vec) => self.eval_array(vec),
            Expression::Binary(BinaryExpression{operator,left,right}) => 
            self.eval_expr(left).and_then(|l| self.eval_expr(right).and_then(|r| self.eval_binary(operator, &l, &r))),
            Expression::Identifier(id) => 
            self.eval_identifier(id),
            Expression::Literal(l) => 
            self.eval_lit(l),
            Expression::Logical(LogicalExpression{operator,left,right}) => self.eval_logic(operator, left, right),
            Expression::Member(MemberExpression {object, property,..}) => self.eval_expr(object).and_then(move|obj| self.eval_expr(property).and_then(move |prop| self.eval_member(&obj, &prop))),
            Expression::Object(props) => 
            self.eval_object(props),
            Expression::Unary(UnaryExpression{operator, argument,..}) => 
            self.eval_expr(argument).and_then(|v|self.eval_unary(operator, &v)),
            Expression::Call(CallExpression {function, argument}) => self.eval_call(function, &self.eval_expr(argument)?),
        }
    }
    
    fn eval_binary<'x>(&self, op: &BinaryOperator, left: &Value<'x>, right: &Value<'x>) -> Result<Value<'e>, EvalError> {
        match op {
            BinaryOperator::StrictEqual => {
                Ok(Value::Boolean(left == right))
            },
            BinaryOperator::StrictNotEqual => {
                Ok(Value::Boolean(left != right))
            },
            BinaryOperator::LessThan => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l < r))
            },
            BinaryOperator::GreaterThan => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l > r))
            },
            BinaryOperator::LessThanEqual => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l <= r))
            },
            BinaryOperator::GreaterThanEqual => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(l >= r))
            },
            BinaryOperator::Plus => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_add(*r).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
            BinaryOperator::Minus => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_sub(*r).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
            BinaryOperator::Times => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_mul(*r).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
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
                l.checked_div(*r).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
            BinaryOperator::Mod => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_rem(*r).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
            BinaryOperator::In => {
                let Value::String(s) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Object(o) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(o.contains_key(s)))
            },
            BinaryOperator::PowerOf => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                l.checked_pow(*r as u32).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::Overflow))
            },
            BinaryOperator::Is => {
                let Value::Type(specified_type) = right else {
                    return Err(EvalError::KindError);
                };
                let actual_type = left.get_type();

                Ok(Value::Boolean(actual_type == *specified_type))
            }
        }
    }

    fn eval_unary(&self, op: &UnaryOperator, arg: &Value) -> Result<Value<'e>, EvalError> {
        match op {
            UnaryOperator::Minus => {
                let Value::Integer(v) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(-v))
            },
            UnaryOperator::Plus => {
                let Value::Integer(v) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(*v))
            },
            UnaryOperator::Not => {
                let Value::Boolean(b) = arg else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Boolean(!b))
            },
        }
    }

    fn eval_object<'x>(&self, props: &'x ObjectExpression<'x>) -> Result<Value<'e>, EvalError> {
        let mut kv_map = BTreeMap::new();

        for prop in props {
            match prop {
                ObjectProperty::Single(id @ Identifier{name}) => {
                    let keyval = Cow::Owned(name.to_string());
                    let valval = self.eval_identifier(id)?;
                    
                    kv_map.insert(keyval, Cow::Owned(valval.to_owned()));
                },
                ObjectProperty::Property(Property{key, value: value_expr}) => {
                    let keyval = match key {
                        PropertyKey::Identifier(Identifier{name}) => Cow::Owned(name.to_string()),
                        PropertyKey::Expression(e) => {
                            let val = self.eval_expr(e)?;
                            let Value::String(s) = val else {
                                return Err(EvalError::TypeError);
                            };
                            s
                        },
                    };
                    let valval = self.eval_expr(value_expr)?;
                    kv_map.insert(keyval, Cow::Owned(valval.to_owned()));

                },
                ObjectProperty::Spread(expr) => {
                    let to_spread = self.eval_expr(expr)?;
                    let Value::Object(map) = to_spread else {
                        return Err(EvalError::TypeError)
                    };
                    for (k,v) in map {
                        kv_map.insert(k, v);
                    }
                },
            }
        };

        Ok(Value::<'e>::Object(kv_map))
    }

    fn eval_array<'x>(&self, vec: &'x [ArrayItem<'x>]) -> Result<Value<'e>, EvalError> {
        let mut result = vec![];

        for item in vec {
            match item {
                ArrayItem::Single(exp) => {
                    let v = self.eval_expr(exp)?;

                    result.push(Cow::Owned(v));
                },
                ArrayItem::Spread(exp) => {
                    let v = self.eval_expr(exp)?;
                    let Value::Array(mut multiples) = v else {
                        return Err(EvalError::TypeError);
                    };

                    result.append(&mut multiples);
                },
            }
        }

        Ok(Value::Array(result))
    }

    fn eval_logic<'x>(&self, operator: &LogicalOperator, left: &'x Expression<'x>, right: &'x Expression<'x>) -> Result<Value<'e>, EvalError> {
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

    fn eval_member<'x:'e>(&self, obj: &Value<'x>, prop: &Value<'x>) -> Result<Value<'x>, EvalError> {
        match obj {
            Value::Object(o) => {
                let Value::String(p) = prop else {
                    return Err(EvalError::TypeError);
                };

                let Some(val) = o.get(p).map(|v|v.clone().into_owned()) else {
                    return Err(EvalError::KeyNotDefined);
                };

                Ok(val)
            },
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
            },
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
            },
            _ => Err(EvalError::TypeError),
        }
    }

    fn eval_identifier(&self, id: &Identifier) -> Result<Value<'e>, EvalError> {
        let Some(val) = self.bindings.get(id) else {
            return Err(EvalError::UnknownIdentifier);
        };

        Ok(val.clone())
    }

    fn eval_call(&self, function: &Identifier, argument: &Value<'e>) -> Result<Value<'e>, EvalError> {
        Ok(match function.name.borrow() {
            Cow::Borrowed("length") => {
                Value::Integer(match argument {
                    Value::String(s) => s.len() as i32,
                    Value::Array(a) => a.len() as i32,
                    Value::Object(o) => o.len() as i32,
                    _ => return Err(EvalError::TypeError) 
                })
            },
            Cow::Borrowed("keys") => {
                Value::Array(match argument {
                    Value::Object(o) => o.keys().map(|k| Cow::Owned(Value::String(Cow::Owned(k.to_string())))).collect(),
                    _ => return Err(EvalError::TypeError) 
                })
            },
            Cow::Borrowed("values") => {
                Value::Array(match argument {
                    Value::Object(o) => o.values().cloned().collect(),
                    _ => return Err(EvalError::TypeError) 
                })
            },
            Cow::Borrowed("type") => {
                Value::Type(argument.get_type())
            }
            _ => return Err(EvalError::UnknownFunction)
        })
    }




}


fn array_item_expression(input: &str) -> IResult<&str, ArrayItem> {
    alt((
        map(preceded(ws(tag("...")), expression), ArrayItem::Spread),
        map(expression, ArrayItem::Single)))
    (input)
}

fn ws<'a, F, O, E: ParseError<&'a str>>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
  where
  F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
  delimited(
    multispace0,
    inner,
    multispace0
  )
}

fn expression_call(input: &str) -> IResult<&str, Expression> {
    map(pair(
        identifier,
        delimited(
            ws(tag("(")),
        expression,
        ws(tag(")")),
        )   
    ),|(function, arg)| 
        Expression::Call(CallExpression {function, argument: Box::new(arg)})
    )(input)
}

fn expression_array(input: &str) -> IResult<&str, Expression> {
    delimited(
        ws(tag("[")),
        terminated(
        map(separated_list0(ws(tag(",")), array_item_expression), Expression::Array),
        opt(ws(tag(",")))),
        ws(tag("]")),
    )(input)
}

fn object_prop_expression(input: &str) -> IResult<&str, ObjectProperty> {
    alt((
        map(
            separated_pair(delimited(
                ws(tag("[")),
                expression,
                ws(tag("]")),
            ), ws(tag(":")), expression), 
            |(prop, value)| ObjectProperty::Property(Property{key: PropertyKey::Expression(prop), value})),
        map(
            separated_pair(identifier, ws(tag(":")), expression), 
            |(prop, value)| ObjectProperty::Property(Property{key: PropertyKey::Identifier(prop), value})),
        map(preceded(ws(tag("...")), expression), ObjectProperty::Spread),
        map(identifier,  ObjectProperty::Single),
    ))(input)
} 

fn expression_object(input: &str) -> IResult<&str, Expression> {
    delimited(
        ws(tag("{")),
        terminated(
        map(separated_list0(ws(ws(tag(","))), object_prop_expression), Expression::Object), 
        opt(ws(tag(",")))),
        ws(tag("}")),
    )(input)
}

fn expression_literal(input: &str) -> IResult<&str, Expression> {
    alt((
        expression_object,
        expression_array,
        expression_call,
        expression_atom,
    ))(input)
}

fn expression_atom(input:&str) -> IResult<&str, Expression> {
    map(alt((
        literal_null,
        literal_string,
        literal_bool,
        literal_number,
        literal_type,
    )), Expression::Literal)(input)
}

fn expression_identifier(input: &str) -> IResult<&str, Expression> {
    map(identifier, Expression::Identifier)(input)
}

fn literal_null(input: &str) -> IResult<&str, Literal> {
    value(Literal::Null, tag("null"))(input)
}

fn literal_string(input: &str) -> IResult<&str, Literal> {
    map(delimited(tag("\""), take_until("\""), tag("\"")), |v| Literal::String(Cow::Borrowed(v)))(input)
}

fn literal_bool(input: &str) -> IResult<&str, Literal> {
    alt((
        value(Literal::Boolean(true), tag("true")),
        value(Literal::Boolean(false), tag("false"))
    ))(input)
}

fn literal_number(input: &str) -> IResult<&str, Literal> {
    map(recognize(i32), |s| Literal::Number(Cow::Borrowed(s)))(input)
}

fn identifier(input: &str) -> IResult<&str, Identifier> {
    map(alpha1, |name| Identifier{name: Cow::Borrowed(name)})(input)
}

fn expression_logic_additive(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_logic_multiplicative(input)?;

    fold_many0(
        pair(ws(alt((
            value(LogicalOperator::Or, tag("||")),
        ))), expression_logic_multiplicative),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}

fn expression_logic_multiplicative(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_type_predicate(input)?;

    fold_many0(
        pair(ws(alt((
            value(LogicalOperator::And, tag("&&")),
        ))), expression_type_predicate),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}

fn literal_type(input: &str) -> IResult<&str, Literal> {
    map(alt((
        value(ValueType::Type, tag("Type")),
        value(ValueType::Null, tag("Null")),
        value(ValueType::Boolean, tag("Boolean")),
        value(ValueType::Integer, tag("Integer")),
        value(ValueType::Array, tag("Array")),
        value(ValueType::Object, tag("Object")),
        value(ValueType::String, tag("String")),
    )), Literal::Type)(input)
}

fn expression_type_predicate(input: &str) -> IResult<&str, Expression>  {
    let (input, init) = expression_numeric_predicative(input)?;

    let Ok((input, t)) = preceded(ws(tag("is")), expression_numeric_predicative)(input) else {
        return Ok((input, init));
    };

    Ok((input, Expression::Binary(
        BinaryExpression { operator: BinaryOperator::Is, 
            left:Box::new(init), 
            right: Box::new(t) 
        })))
}

fn expression_numeric_predicative(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_numeric_additive(input)?;

    fold_many0(
        pair(ws(alt((
            value(BinaryOperator::GreaterThanEqual, tag(">=")),
            value(BinaryOperator::LessThanEqual, tag("<=")),
            value(BinaryOperator::LessThan, char('<')),
            value(BinaryOperator::GreaterThan, char('>')),
            value(BinaryOperator::StrictEqual, tag("==")),
            value(BinaryOperator::StrictNotEqual, tag("!=")),
            value(BinaryOperator::In, tag("in")),
        ))), expression_numeric_additive),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression { operator, left:Box::new(left), right: Box::new(right) })
        },
      )(input)
}

fn expression_numeric_additive(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_numeric_multiplicative(input)?;

    fold_many0(
        pair(ws(alt((
            value(BinaryOperator::Plus, char('+')),
            value(BinaryOperator::Minus, char('-')),
        ))), expression_numeric_multiplicative),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}

fn expression_numeric_multiplicative(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_numeric_exponential(input)?;

    fold_many0(
        pair(ws(alt((
            value(BinaryOperator::Times, char('*')),
            value(BinaryOperator::Over, char('/')),
            value(BinaryOperator::Mod, char('%')),
        ))), expression_numeric_exponential),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}

fn expression_numeric_exponential(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_indexed(input)?;

    fold_many0(
        pair(ws(alt((
            value(BinaryOperator::PowerOf, char('^')),
        ))), expression_indexed),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}


fn expression_indexed(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_member(input)?;

    fold_many0(
        delimited(ws(tag("[")), expression, ws(tag("]"))),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression{object:Box::new(acc), property:Box::new(ident)})
        },
      )(input)
}

fn expression_member(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_primary(input)?;

    fold_many0(
        alt((
            preceded(ws(char('.')), identifier),
        )),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression{object:Box::new(acc), property:Box::new(Expression::Literal(Literal::String(ident.name)))})
        },
      )(input)
}

fn expression_primary(input: &str) -> IResult<&str, Expression> {
    alt((
        expression_with_paren,
        expression_literal,
        expression_identifier,
        expression_unary,
    ))(input)
}

fn expression_with_paren(input: &str) -> IResult<&str, Expression> {
    delimited(tag("("), expression, tag(")"))(input)
}

fn expression_unary(input: &str) -> IResult<&str, Expression> {
    alt((expression_unary_logic, expression_unary_numeric))(input)
}

fn expression_unary_logic(input: &str) -> IResult<&str, Expression> {
    map(pair(ws(alt((
        value(UnaryOperator::Not, tag("!")),
    ))), expression_primary), 
    |(operator, argument)| 
    Expression::Unary(UnaryExpression{operator, argument: Box::new(argument)}))(input)
}

fn expression_unary_numeric(input: &str) -> IResult<&str, Expression> {
    map(pair(ws(alt((
        value(UnaryOperator::Minus, tag("-")),
        value(UnaryOperator::Plus, tag("+")),
    ))), alt((
        expression_indexed,
    ))), |(operator, argument)| 
    Expression::Unary(UnaryExpression{operator, argument: Box::new(argument)}))(input)
}

fn expression(input: &str) -> IResult<&str, Expression> {
    alt((
        expression_logic_additive,
    ))(input)
}

fn full_expression(input: &str) -> IResult<&str, Expression> {
    all_consuming(expression)(input)
}

fn pattern_discard(input: &str) -> IResult<&str, Pattern> {
    value(Pattern::Discard, tag("_"))(input)
}


fn pattern_identifier(input: &str) -> IResult<&str, Pattern> {
    map(identifier, Pattern::Identifier)(input)
}



fn object_prop_pattern(input: &str) -> IResult<&str, ObjectPropertyPattern> {
    alt((
        map(
            separated_pair(delimited(
                ws(tag("[")),
                expression,
                ws(tag("]")),
            ), ws(tag(":")), pattern), 
            |(prop, value)| ObjectPropertyPattern::Match(PropertyPattern{key: PropertyKey::Expression(prop), value})),
        map(
            separated_pair(identifier, ws(tag(":")), pattern), 
            |(prop, value)| ObjectPropertyPattern::Match(PropertyPattern{key: PropertyKey::Identifier(prop), value})),
        map(identifier,  ObjectPropertyPattern::Single),
    ))(input)
} 

fn pattern_object(input: &str) -> IResult<&str, Pattern> {
    delimited(
        ws(tag("{")),
        alt((
            map(pattern_rest, |r| Pattern::Object(vec![], r)),
            map(tuple((
                separated_list0(ws(ws(tag(","))), object_prop_pattern),
                opt(preceded(ws(tag(",")), pattern_rest)),
            )), |(props,rest)| Pattern::Object(props, rest.unwrap_or(Rest::Discard)))
        )),
        ws(tag("}")),
    )(input)
}

fn pattern_rest(input: &str) -> IResult<&str, Rest> {
    alt((
        map(preceded(ws(tag("...")), pattern), |r| Rest::Collect(Box::new(r))),
        value(Rest::Discard, ws(tag("..."))),
    ))(input)
}

fn pattern_array(input: &str) -> IResult<&str, Pattern> {
    delimited(
        ws(tag("[")),
        alt((
            map(pattern_rest, |r| Pattern::Array(vec![], r)),
            map(tuple((
                separated_list0(
                    ws(tag(",")), 
                    map(pattern, ArrayPatternItem::Pattern)),
                opt(preceded(ws(tag(",")), pattern_rest)))), |(items,rest)| Pattern::Array(items, rest.unwrap_or(Rest::Exact))),
        )),
        ws(tag("]")),
    )(input)
}

fn pattern(input: &str) -> IResult<&str, Pattern> {
    alt((
        pattern_array,
        pattern_discard,
        pattern_identifier,
        pattern_object,
    ))(input)
}

fn full_pattern(input: &str) -> IResult<&str, Pattern> {
    all_consuming(pattern)(input)
}

fn full_matching(input: &str) -> IResult<&str, (Pattern, Expression)> {
    all_consuming(separated_pair(pattern, ws(tag("=")), expression))(input)
}

enum Statement<'a,'b> {
    Inspect(Expression<'b>),
    Format(Expression<'b>),
    Eval(Expression<'b>),
    Assign(Pattern<'a>, Expression<'b>),
}

fn assignment(input:&str) -> IResult<&str, Statement> {
    map(separated_pair(
        pattern, 
        ws(tag(":=")), 
        full_expression), 
        |(pat, expr)| Statement::Assign(pat, expr))(input)
}

fn statement(input:&str) -> IResult<&str, Statement> {
    alt((
        map(preceded(tag(".inspect "), full_expression), Statement::Inspect),
        map(preceded(tag(".format "), full_expression), Statement::Format),
        assignment,
        map(full_expression, Statement::Eval),
    ))(input)
}

fn main() -> rustyline::Result<()>{
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
                    },
                };

                match stmt {
                    Statement::Inspect(ex) => {
                        dbg!(ex);
                    },
                    Statement::Format(ex) => {
                        println!("{ex:?}");
                    },
                    Statement::Eval(ex) => {
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            },
                        };

                        println!("{result}");

                    },
                    Statement::Assign(pattern, ex) => {
                        let mut matcher = Matcher {
                            env: &env,
                            bindings: BTreeMap::new(),
                        };
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            },
                        };
                        //matcher.bindings.insert(Identifier { name: "c".into() }, result.clone());
                        //matcher.match_pattern(&pattern, result.clone());

                        println!("{pattern} := {result}");
                    }
                };


            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {err}");
                break
            }
        }
    }
    rl.save_history("history.txt")
    
}

#[cfg(test)]
mod test {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_expressions() {
        let tests = include_str!("test_expressions.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for [expr, result,sep] in tests.into_iter().array_chunks() {
            assert_eq!("---", sep);
            let parsed = full_expression(expr);
            let value = full_expression(result);
            assert!(parsed.is_ok());
            
            assert!(value.is_ok());

            let evaled = env.eval_expr(&parsed.unwrap().1);
            let valued_evaled = env.eval_expr(&value.unwrap().1);

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
        let mut matcher = Matcher {
            env: &env,
            bindings: BTreeMap::new(),
        };

        for case in tests {
            matcher.clear();
            let Ok((_, (pattern, expr))) = full_matching(case) else {
                dbg!(case);
                unreachable!();
            };

            let Ok(value) = env.eval_expr(&expr) else {
                unreachable!();
            };
            dbg!(case);

            assert!(matcher.match_pattern(&pattern, value));
        }

    }

    #[test]
    fn test_negative_patterns() {
        let tests = include_str!("test_negative_patterns.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };
        let mut matcher = Matcher {
            env: &env,
            bindings: BTreeMap::new(),
        };


        for case in tests {
            matcher.clear();
            let Ok((_, (pattern, expr))) = full_matching(case) else {
                dbg!(case);
                unreachable!();
            };

            let Ok(value) = env.eval_expr(&expr) else {
                unreachable!();
            };
            dbg!(case);

            assert!(!matcher.match_pattern(&pattern, value));
        }

    }
}
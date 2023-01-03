#![feature(iter_array_chunks)]

use std::borrow::{Cow};
use std::collections::BTreeMap;
use nom::IResult;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{multispace0, i32, alpha1, char};
use nom::combinator::{map, value, recognize, all_consuming, opt};
use nom::error::ParseError;
use nom::multi::{separated_list0, fold_many0};
use nom::sequence::{delimited, separated_pair, preceded, pair, terminated};
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
        };
        write!(f,"")
    }
}

enum Pattern<'a> {
    Identifier(Identifier<'a>),
    Object(ObjectPattern<'a>),
    Array(),
    RestElement(Box<Pattern<'a>>),
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
struct Identifier<'a> {
    name: Cow<'a, str>,
}

type ObjectPattern<'a> = Vec<ObjectPatternPart<'a>>;
type ArrayPattern<'a> = Vec<ArrayPatternPart<'a>>;

enum ArrayPatternPart<'a> {
    Pattern(Pattern<'a>),
    Expression(Expression<'a>),
}

enum ObjectPatternPart<'a> {
    Assignment(Property<'a>),
    Rest(Box<Pattern<'a>>),
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
}



struct Environment<'e> {
    bindings: BTreeMap<Identifier<'e>, Value<'e>>
}

#[derive(Debug)]
enum EvalError {
    TypeError,
    UnknownIdentifier,
    InvalidNumber,
    MathDivision,
    KeyNotDefined,
    OutOfBound,
}

impl <'e> Environment<'e> {
    fn eval_lit<'x>(&self, literal: &'x Literal<'x>) -> Result<Value<'e>, EvalError> {
        match literal {
            Literal::Null => Ok(Value::Null),
            Literal::String(s) => Ok(Value::<'e>::String(Cow::Owned(s.to_string()))),
            Literal::Number(s) => str::parse::<i32>(s).map(Value::Integer).map(Ok).unwrap_or(Err(EvalError::InvalidNumber)),
            Literal::Boolean(b) => Ok(Value::Boolean(*b)),
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
                Ok(Value::Integer(*l + *r))
            },
            BinaryOperator::Minus => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(*l - *r))
            },
            BinaryOperator::Times => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(*l * *r))
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
                Ok(Value::Integer(*l / *r))
            },
            BinaryOperator::Mod => {
                let Value::Integer(l) = left else {
                    return Err(EvalError::TypeError);
                };
                let Value::Integer(r) = right else {
                    return Err(EvalError::TypeError);
                };
                Ok(Value::Integer(*l % *r))
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
                Ok(Value::Integer(i32::pow(*l,*r as u32)))
            },
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
            _ => Err(EvalError::TypeError),
        }
    }

    fn eval_identifier(&self, id: &Identifier) -> Result<Value<'e>, EvalError> {
        let Some(val) = self.bindings.get(id) else {
            return Err(EvalError::UnknownIdentifier);
        };

        Ok(val.clone())
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
        expression_atom,
    ))(input)
}

fn expression_atom(input:&str) -> IResult<&str, Expression> {
    map(alt((
        literal_null,
        literal_string,
        literal_bool,
        literal_number,
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
    let (input, init) = expression_numeric_predicative(input)?;

    fold_many0(
        pair(ws(alt((
            value(LogicalOperator::And, tag("&&")),
        ))), expression_numeric_predicative),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression { operator, left:Box::new(left), right: Box::new(right) })

        },
      )(input)
}

fn expression_numeric_predicative(input: &str) -> IResult<&str, Expression> {
    let (input, init) = expression_numeric_additive(input)?;

    fold_many0(
        pair(ws(alt((
            value(BinaryOperator::LessThan, char('<')),
            value(BinaryOperator::LessThanEqual, tag("<=")),
            value(BinaryOperator::GreaterThan, char('>')),
            value(BinaryOperator::GreaterThanEqual, tag(">=")),
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
    ))), expression_primary), |(operator, argument)| Expression::Unary(UnaryExpression{operator, argument: Box::new(argument)}))(input)
}

fn expression_unary_numeric(input: &str) -> IResult<&str, Expression> {
    map(pair(ws(alt((
        value(UnaryOperator::Minus, tag("-")),
        value(UnaryOperator::Plus, tag("+")),
    ))), alt((
        expression_indexed,
    ))), |(operator, argument)| Expression::Unary(UnaryExpression{operator, argument: Box::new(argument)}))(input)
}

fn expression(input: &str) -> IResult<&str, Expression> {
    alt((
        expression_logic_additive,
    ))(input)
}

fn full_expression(input: &str) -> IResult<&str, Expression> {
    all_consuming(expression)(input)
}

enum Statement<'a> {
    Inspect(Expression<'a>),
    Format(Expression<'a>),
    Eval(Expression<'a>),
    Assign(Identifier<'a>, Expression<'a>),
}

fn assignment(input:&str) -> IResult<&str, Statement> {
    map(separated_pair(
        identifier, 
        ws(tag("=")), 
        full_expression), 
        |(id, expr)| Statement::Assign(id, expr))(input)
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
                    Statement::Assign(Identifier{name}, ex) => {
                        let result = match env.eval_expr(&ex) {
                            Ok(r) => r,
                            Err(err) => {
                                println!("Eval Error, {err:?}");
                                continue;
                            },
                        };
                        env.bindings.insert(Identifier { name: Cow::Owned(name.to_string()) }, result.to_owned());

                        println!("{result}");
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
    use super::*;

    #[test]
    fn test_many() {
        let tests = include_str!("tests.txt").lines();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for [expr, result] in tests.into_iter().array_chunks() {
            let parsed = full_expression(expr);
            let value = expression_atom(result);

            assert!(parsed.is_ok());
            assert!(value.is_ok());

            let evaled = env.eval_expr(&parsed.unwrap().1);
            let valued_evaled = env.eval_expr(&value.unwrap().1);

            assert!(evaled.is_ok());
            assert!(valued_evaled.is_ok());

            assert_eq!(evaled.unwrap(), valued_evaled.unwrap());
        }

    }
}
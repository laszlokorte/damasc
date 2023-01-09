use std::{borrow::Cow, collections::BTreeMap};

use crate::{
    expression::*,
    identifier::Identifier,
    literal::Literal,
    value::{Value, ValueType},
};

#[derive(Clone, Debug)]
pub(crate) struct Environment<'i, 's, 'v> {
    pub(crate) bindings: BTreeMap<Identifier<'i>, Value<'s, 'v>>,
}

#[derive(Debug)]
pub(crate) enum EvalError {
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

impl<'i, 's, 'v> Environment<'i, 's, 'v> {
    pub(crate) fn clear(&mut self) {
        self.bindings.clear();
    }

    pub(crate) fn eval_expr<'x>(
        &self,
        expression: &'x Expression<'x>,
    ) -> Result<Value<'s, 'v>, EvalError> {
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
            Expression::Template(template) => self.eval_template(template),
        }
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

    fn eval_binary(
        &self,
        op: &BinaryOperator,
        left: &Value<'s, 'v>,
        right: &Value<'s, 'v>,
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
            BinaryOperator::Cast => {
                let Value::Type(specified_type) = right else {
                    return Err(EvalError::KindError);
                };

                let Some(v) = left.convert(*specified_type) else {
                    return Err(EvalError::TypeError);
                };

                Ok(v)
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

    fn eval_template<'x>(
        &self,
        template: &'x StringTemplate<'x>,
    ) -> Result<Value<'s, 'v>, EvalError> {
        let joined = template
            .parts
            .iter()
            .flat_map(move |part| {
                let prefix = Ok(Cow::Owned(part.fixed_start.as_ref().into()));

                match self
                    .eval_expr(&part.dynamic_end)
                    .map(|v| v.convert(ValueType::String))
                {
                    Ok(Some(Value::String(end))) => [prefix, Ok(end)],
                    Ok(_) => [prefix, Err(EvalError::TypeError)],
                    Err(e) => [prefix, Err(e)],
                }
            })
            .chain(Some(Ok(Cow::Owned(template.suffix.as_ref().into()))))
            .collect::<Result<Vec<Cow<'s, str>>, _>>()?;

        return Ok(Value::String(Cow::Owned(joined.join(""))));
    }

    pub(crate) fn identifiers(&self) -> std::collections::HashSet<&Identifier> {
        self.bindings.keys().collect()
    }
}

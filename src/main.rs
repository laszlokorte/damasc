#![feature(iter_array_chunks)]
#![feature(assert_matches)]
#![feature(map_try_insert)]
#![feature(let_chains)]

use std::borrow::Cow;
use std::collections::BTreeMap;

use rustyline::error::ReadlineError;
use rustyline::Editor;

mod env;
mod expression;
mod identifier;
mod matcher;
mod pattern;
mod value;
mod statement;
mod parser;

use env::Environment;
use expression::Literal;
use expression::*;
use identifier::Identifier;
use matcher::Matcher;
use value::Value;
use statement::Statement;
use parser::statement;

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
                                matcher.apply_to_env(&mut env);
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
    use crate::parser::{full_expression, try_match};
    use super::*;

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

            let Ok((_, Statement::Match(pattern, expr))) = try_match(case) else {
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
            let Ok((_, Statement::Match(pattern, expr))) = try_match(case) else {
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

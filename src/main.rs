#![feature(iter_array_chunks)]
#![feature(assert_matches)]
#![feature(map_try_insert)]
#![feature(let_chains)]
#![feature(generators)]

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};

use literal::Literal;
use rustyline::error::ReadlineError;
use rustyline::Editor;

mod bag;
mod typed_bag;
mod env;
mod expression;
mod identifier;
mod literal;
mod matcher;
mod parser;
mod pattern;
mod query;
mod statement;
mod value;
mod assignment;

use env::{Environment, EvalError};
use expression::*;
use identifier::Identifier;
use matcher::Matcher;
use parser::{full_expression, statement};
use statement::Statement;
use value::Value;

use crate::assignment::{Assignment, AssignmentSet};
use crate::query::Predicate;
use crate::typed_bag::TypedBag;

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

const INITIAL_BAG_NAME : &str = "init";

fn main() -> rustyline::Result<()> {
    let mut env = Environment {
        bindings: BTreeMap::new(),
    };

    let mut current_bag_name = Identifier { name: Cow::Borrowed(INITIAL_BAG_NAME) };
    let mut bags = HashMap::<Identifier, TypedBag>::new();
    bags.insert(current_bag_name.clone(), crate::typed_bag::TypedBag::new(Predicate {
        pattern: crate::parser::pattern("_").unwrap().1,
        guard: full_expression("true").unwrap().1,
        limit: None,
    })); 

    let mut rl = Editor::<()>::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }

    println!("Welcome");
    println!("press CTRL-D to exit.");
    println!(".bag");
    println!("Current Bag: {current_bag_name}");

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
                    },
                    Statement::Exit => {
                        break;
                    },
                    Statement::Help => {
                        println!("Interactive help is not yet implemented. Please take a look at the README.md file");
                    },
                    Statement::TellBag => {
                        println!("Current Bag: {current_bag_name}");
                    },
                    Statement::UseBag(bag_id, pred) => {
                        current_bag_name = bag_id;
                        let wants_create = pred.is_some();
                        if bags.try_insert(current_bag_name.clone(), crate::typed_bag::TypedBag::new(pred.unwrap_or(Predicate {
                            pattern: crate::parser::pattern("_").unwrap().1,
                            guard: full_expression("true").unwrap().1,
                            limit: None,
                        }))).is_ok(){
                            println!("CREATED BAG");
                        } else {
                            if wants_create {
                                println!("BAG ALREADY EXISTS");
                            }
                            println!("SWITCHED BAG");
                        };
                    },
                    Statement::Import(filename) => {             
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        let Ok(file) = File::open(filename.as_ref()) else {
                            println!("No file: {filename}");
                            continue;
                        };
                        let lines = io::BufReader::new(file).lines();

                        for l in lines {
                            let Ok(line) = l else {
                                println!("Read error: {line}");
                                continue;
                            };
                            let Ok((_, expr)) = full_expression(&line) else {
                                println!("Parse error: {line}");
                                continue;
                            };
                            let Ok(value) = env.eval_expr(&expr) else {
                                println!("Eval error: {line}");
                                continue;
                            };
                            bag.insert(&value);
                        }

                        println!("Imported values from file '{filename}' into current bag({current_bag_name})");
                    }
                    Statement::Export(filename) => {
                        use std::io::Write;          
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        let Ok(file) = File::create(filename.as_ref()) else {
                            println!("File {filename} could not be created");
                            continue;
                        };
                        {
                            let mut file = LineWriter::new(file);
                            for v in bag.iter() {
                                let _ = writeln!(file, "{v}");
                            }
                        }
                        println!("Current bag({current_bag_name}) written to file: {filename}");
                    }
                    Statement::Insert(expressions) => {          
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        let values: Result<Vec<Value>, EvalError> =
                            expressions.into_iter().map(|e| env.eval_expr(&e)).collect();
                        match values {
                            Ok(values) => {
                                let mut counter = 0;
                                for v in values {
                                    if bag.insert(&v) {
                                        counter += 1;
                                    } else {
                                        break;
                                    }
                                }
                                if counter > 0 {
                                    println!("INSERTED {counter}");
                                } else {
                                    println!("NO");
                                }
                            }
                            Err(e) => {
                                println!("Eval Error: {e:?}");
                            }
                        }
                    }
                    Statement::Query(query) => {          
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        for projected in bag.query(&env, &query) {
                            match projected {
                                Ok(v) => println!("{v}"),
                                Err(e) => println!("Error: {e:?}"),
                            }
                        }
                    }
                    Statement::Deletion(predicate) => {          
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        let count = bag.delete(&env, &predicate);
                        if count > 0 {
                            println!("DELETED {count}");
                        } else {
                            println!("NO");
                        }
                    }
                    Statement::Pop(expression) => {
                        let Some(bag) = bags.get_mut(&current_bag_name) else {
                            continue;
                        };

                        let Ok(value) = env.eval_expr(&expression) else {
                            continue;
                        };
                        if bag.pop(&value) {
                            println!("OK")
                        } else {
                            println!("NO");
                        }
                    }

                    Statement::Inspect(ex) => {
                        dbg!(ex);
                    }
                    Statement::Format(ex) => {
                        println!("{ex:?}");
                    }

                    Statement::Eval(ExpressionSet{ expressions }) => {
                        for expression in expressions {
                            let result = match env.eval_expr(&expression) {
                                Ok(r) => r,
                                Err(err) => {
                                    println!("Eval Error, {err:?}");
                                    continue;
                                }
                            };

                            println!("{result}");
                        }
                    }
                    Statement::MatchSet(mut assignments) => {
                        assignments.sort_topological();
                        
                        for Assignment { pattern, expression } in assignments.assignments {
                            let mut matcher = Matcher {
                                env: &env.clone(),
                                bindings: BTreeMap::new(),
                            };
                            let result = match env.eval_expr(&expression) {
                                Ok(r) => r,
                                Err(err) => {
                                    println!("Eval Error, {err:?}");
                                    continue;
                                }
                            };
    
                            match matcher.match_pattern(&pattern, &result) {
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
                    }
                    Statement::AssignSet(mut assignments) => {
                        assignments.sort_topological();
                        
                        for Assignment { pattern, expression } in assignments.assignments {
                            let mut matcher = Matcher {
                                env: &env.clone(),
                                bindings: BTreeMap::new(),
                            };
                            let result = match env.eval_expr(&expression) {
                                Ok(r) => r,
                                Err(err) => {
                                    println!("Eval Error, {err:?}");
                                    continue;
                                }
                            };
    
                            match matcher.match_pattern(&pattern, &result) {
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
                continue;
            }
            Err(ReadlineError::Eof) => {
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
    use super::*;
    use crate::parser::{expression_multi, try_match_multi};
    use std::assert_matches::assert_matches;

    #[test]
    fn test_expressions() {
        let mut tests = include_str!("test_expressions.txt").lines().array_chunks();
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        for [expr, result, sep] in &mut tests {
            assert_eq!("---", sep, "Expression pairs are separated by --- line");
            let Ok((_, parsed)) = expression_multi(expr) else {
                unreachable!("Expression set A can be parsed, {expr}");
            };
            let Ok((_, value)) = expression_multi(result) else {
                unreachable!("Expression set B can be parsed, {result}");
            };
            
            for (a,b) in std::iter::zip(parsed.expressions, value.expressions) {
                let evaled = env.eval_expr(&a);
                let valued_evaled = env.eval_expr(&b);

                assert!(evaled.is_ok(), "Expression A can be evaluated");
                assert!(valued_evaled.is_ok(), "Expression B can be parsed");

                assert_eq!(
                    evaled.unwrap(),
                    valued_evaled.unwrap(),
                    "Expression A and B evaluate to the same value"
                );
            }
            
        }

        let Some(e) = tests.into_remainder() else {
            unreachable!("Number of Test Expression lines are multiple of 3");
        };
        assert_eq!(
            e.count(),
            0,
            "Last expression pair is followed terminated by ---"
        );
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

            let Ok((_, Statement::MatchSet(AssignmentSet { assignments }))) = try_match_multi(case) else {
                unreachable!("Test Pattern and Expression can be parsed: {case}");
            };

            for Assignment{pattern, expression} in assignments {  

                let Ok(value) = env.eval_expr(&expression) else {
                    unreachable!("TestExpression can be evaluated: {case}");
                };

                assert_matches!(
                    matcher.match_pattern(&pattern, &value),
                    Ok(_),
                    "Test Expression Value matches the test pattern: {case}"
                );
            }
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
            let Ok((_, Statement::MatchSet(AssignmentSet{assignments}))) = try_match_multi(case) else {
                dbg!(case);
                unreachable!("Test Pattern and Expression can be parsed: {case}");
            };

            for Assignment{pattern, expression} in assignments {  
                let Ok(value) = env.eval_expr(&expression) else {
                    unreachable!("TestExpression can be evaluated: {case}");
                };
                dbg!(case);

                assert_matches!(
                    matcher.match_pattern(&pattern, &value),
                    Err(_),
                    "Test Expression Value does not match the test pattern: {case}"
                );
            }
        }
    }
}

#![feature(map_try_insert)]

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};
use rustyline::error::ReadlineError;
use rustyline::Editor;

use damasc::typed_bag::TypedBag;
use damasc::env::{Environment, EvalError};
use damasc::expression::*;
use damasc::identifier::Identifier;
use damasc::matcher::Matcher;
use damasc::parser::{full_expression, statement, pattern};
use damasc::statement::Statement;
use damasc::value::Value;

use damasc::assignment::Assignment;
use damasc::query::Predicate;

const INITIAL_BAG_NAME: &str = "init";

pub(crate) fn main() -> rustyline::Result<()> {
    let mut env = Environment {
        bindings: BTreeMap::new(),
    };

    let mut current_bag_name = Identifier {
        name: Cow::Borrowed(INITIAL_BAG_NAME),
    };
    let mut bags = HashMap::<Identifier, TypedBag>::new();
    bags.insert(
        current_bag_name.clone(),
        TypedBag::new(Predicate {
            pattern: pattern("_").unwrap().1,
            guard: full_expression("true").unwrap().1,
            limit: None,
        }),
    );

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
                    }
                    Statement::Exit => {
                        break;
                    }
                    Statement::Help => {
                        println!("Interactive help is not yet implemented. Please take a look at the README.md file");
                    }
                    Statement::TellBag => {
                        println!("Current Bag: {current_bag_name}");
                    }
                    Statement::UseBag(bag_id, pred) => {
                        current_bag_name = bag_id;
                        let wants_create = pred.is_some();
                        if bags
                            .try_insert(
                                current_bag_name.clone(),
                                TypedBag::new(pred.unwrap_or(Predicate {
                                    pattern: pattern("_").unwrap().1,
                                    guard: full_expression("true").unwrap().1,
                                    limit: None,
                                })),
                            )
                            .is_ok()
                        {
                            println!("CREATED BAG");
                        } else {
                            if wants_create {
                                println!("BAG ALREADY EXISTS");
                            }
                            println!("SWITCHED BAG");
                        };
                    }
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

                    Statement::Eval(ExpressionSet { expressions }) => {
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
                        if let Err(e) = assignments.sort_topological(env.identifiers()) {
                            println!("Assignment Error: {e}");
                            continue;
                        }
                        let mut tmp_env = env.clone();

                        for Assignment {
                            pattern,
                            expression,
                        } in assignments.assignments
                        {
                            let mut matcher = Matcher {
                                env: &tmp_env.clone(),
                                bindings: BTreeMap::new(),
                            };

                            let result = match tmp_env.eval_expr(&expression) {
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

                                    matcher.apply_to_env(&mut tmp_env);
                                }
                                Err(e) => {
                                    println!("NO: {e:?}")
                                }
                            }
                        }
                    }
                    Statement::AssignSet(mut assignments) => {
                        if let Err(e) = assignments.sort_topological(env.identifiers()) {
                            println!("Assignment Error: {e}");
                            continue;
                        }

                        let mut matcher = Matcher {
                            env: &env.clone(),
                            bindings: BTreeMap::new(),
                        };

                        for Assignment {
                            pattern,
                            expression,
                        } in assignments.assignments
                        {
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

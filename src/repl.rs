use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};

use crate::env::{Environment, EvalError};
use crate::expression::*;
use crate::identifier::Identifier;
use crate::matcher::Matcher;
use crate::parser::{full_expression, pattern};
use crate::statement::Statement;
use crate::typed_bag::TypedBag;
use crate::value::Value;

use crate::assignment::Assignment;
use crate::query::Predicate;

pub struct Repl<'i, 's, 'v> {
    pub env: Environment<'i, 's, 'v>,
    pub current_bag: Identifier<'s>,
    pub bags: HashMap<Identifier<'s>, TypedBag<'i, 's, 'v>>,
}

#[derive(Debug)]
pub enum ReplOutput<'s, 'v> {
    Ack,
    No,
    PatternMissmatch,
    Values(Vec<Value<'s, 'v>>),
    Bindings(HashMap<Identifier<'s>, Value<'s, 'v>>),
    Deleted(usize),
    Inserted(usize),
    Notice(String),
}

impl<'s, 'v> std::fmt::Display for ReplOutput<'s, 'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplOutput::Ack => writeln!(f, "OK."),
            ReplOutput::No => writeln!(f, "NO."),
            ReplOutput::Values(values) => {
                for v in values {
                    let _ = writeln!(f, "{v};");
                }
                write!(f, "")
            }
            ReplOutput::Bindings(b) => {
                let _ = writeln!(f, "YES.");
                for (k, v) in b.iter() {
                    let _ = writeln!(f, "{k} := {v};");
                }
                write!(f, "")
            }
            ReplOutput::Deleted(count) => writeln!(f, "DELETED {count}."),
            ReplOutput::Inserted(count) => writeln!(f, "INSERTED {count}."),
            ReplOutput::Notice(n) => writeln!(f, "{n}"),
            ReplOutput::PatternMissmatch => writeln!(f, "NO."),
        }
    }
}

#[derive(Debug)]
pub enum ReplError {
    ReadError,
    ParseError,
    EvalError,
    AssignmentError,
    IoError,
    Exit,
    BagError,
}

impl<'i, 's, 'v> Repl<'i, 's, 'v> {
    pub fn new(initial_bag: &'s str) -> Self {
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        let current_bag = Identifier {
            name: Cow::Borrowed(initial_bag),
        };
        let mut bags = HashMap::<Identifier, TypedBag>::new();
        bags.insert(
            current_bag.clone(),
            TypedBag::new(Predicate {
                pattern: pattern("_").unwrap().1,
                guard: full_expression("true").unwrap().1,
                limit: None,
            }),
        );

        Self {
            env,
            current_bag,
            bags,
        }
    }

    pub fn execute(&mut self, stmt: Statement<'s, 's>) -> Result<ReplOutput<'s, 'v>, ReplError> {
        match stmt {
            Statement::Noop => {
                self.env.clear();
                Ok(ReplOutput::Ack)
            }
            Statement::Clear => {
                self.env.clear();
                Ok(ReplOutput::Ack)
            }
            Statement::Exit => Err(ReplError::Exit),
            Statement::Help => {
                return Ok(ReplOutput::Notice("Interactive help is not yet implemented. Please take a look at the README.md file".to_string()));
            }
            Statement::TellBag => {
                return Ok(ReplOutput::Notice(format!(
                    "Current Bag: {}, size: {}",
                    self.current_bag,
                    self.bags.get(&self.current_bag).map(TypedBag::len).unwrap_or(0)
                )));
            }
            Statement::ListBags => {
                return Ok(ReplOutput::Notice(format!(
                    "Bags: {}",
                    self.bags.keys().map(|i|i.name.as_ref()).collect::<Vec<_>>().join(", ")
                )));
            }
            Statement::UseBag(bag_id, pred) => {
                self.current_bag = bag_id.clone();
                let wants_create = pred.is_some();
                if self
                    .bags
                    .try_insert(
                        self.current_bag.clone(),
                        TypedBag::new(pred.clone().unwrap_or(Predicate {
                            pattern: pattern("_").unwrap().1,
                            guard: full_expression("true").unwrap().1,
                            limit: None,
                        })),
                    )
                    .is_ok()
                {
                    return Ok(ReplOutput::Notice("BAG CREATED".into()));
                } else if wants_create {
                    return Ok(ReplOutput::Notice("ALREADY EXISTS, SWITCHED BAG".into()));
                } else {
                    return Ok(ReplOutput::Notice("SWITCHED BAG".into()));
                };
            }
            Statement::Import(filename) => {
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                let Ok(file) = File::open(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                let lines = io::BufReader::new(file).lines();

                for l in lines {
                    let Ok(line) = l else {
                        return Err(ReplError::ReadError);
                    };
                    let Ok((_, expr)) = full_expression(&line) else {
                        return Err(ReplError::ParseError);
                    };
                    let Ok(value) = self.env.eval_expr(&expr) else {
                        return Err(ReplError::EvalError);
                    };
                    bag.insert(&value);
                }

                return Ok(ReplOutput::Notice(format!(
                    "Imported values from file '{filename}' into current bag({})",
                    self.current_bag
                )));
            }
            Statement::Export(filename) => {
                use std::io::Write;
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                let Ok(file) = File::create(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                {
                    let mut file = LineWriter::new(file);
                    for v in bag.iter() {
                        let _ = writeln!(file, "{v}");
                    }
                }
                return Ok(ReplOutput::Notice(format!(
                    "Current bag({}) written to file: {filename}",
                    self.current_bag
                )));
            }
            Statement::Insert(expressions) => {
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                let values: Result<Vec<Value>, EvalError> = expressions
                    .into_iter()
                    .map(|e| self.env.eval_expr(&e))
                    .collect();
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
                            return Ok(ReplOutput::Inserted(counter));
                        } else {
                            Ok(ReplOutput::No)
                        }
                    }
                    Err(_e) => Err(ReplError::EvalError),
                }
            }
            Statement::Query(query) => {
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                return bag
                    .query(&self.env, &query)
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .map(ReplOutput::Values)
                    .map_err(|_| ReplError::EvalError);
            }
            Statement::Deletion(predicate) => {
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                let count = bag.delete(&self.env, &predicate);
                if count > 0 {
                    return Ok(ReplOutput::Deleted(count));
                } else {
                    Ok(ReplOutput::No)
                }
            }
            Statement::Pop(expression) => {
                let Some(bag) = self.bags.get_mut(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                let Ok(value) = self.env.eval_expr(&expression) else {
                    return Err(ReplError::EvalError);
                };
                if bag.pop(&value) {
                    Ok(ReplOutput::Ack)
                } else {
                    Ok(ReplOutput::No)
                }
            }
            Statement::Inspect(ex) => {
                return Ok(ReplOutput::Notice(format!("{ex:?}")));
            }
            Statement::Format(ex) => {
                return Ok(ReplOutput::Notice(format!("{ex:?}")));
            }

            Statement::Eval(ExpressionSet { expressions }) => expressions
                .into_iter()
                .map(|e| self.env.eval_expr(&e).map_err(|_| ReplError::EvalError))
                .collect::<Result<Vec<_>, _>>()
                .map(ReplOutput::Values)
                .map_err(|_| ReplError::EvalError),
            Statement::MatchSet(mut assignments) => {
                if let Err(_e) = assignments.sort_topological(self.env.identifiers()) {
                    return Err(ReplError::AssignmentError);
                }

                let mut bindings = HashMap::new();
                let result = assignments.assignments.iter().fold(
                    Ok(Ok(self.env.clone())),
                    |acc,
                     Assignment {
                         pattern,
                         expression,
                     }| {
                        let Ok(Ok(mut tmp_env)) = acc else {
                        return acc;
                    };

                        let mut matcher = Matcher {
                            env: &tmp_env.clone(),
                            bindings: BTreeMap::new(),
                        };

                        let result = match tmp_env.eval_expr(expression) {
                            Ok(r) => r,
                            Err(_err) => {
                                return Err(ReplError::EvalError);
                            }
                        };

                        match matcher.match_pattern(pattern, &result) {
                            Ok(_) => {
                                for (id, v) in &matcher.bindings {
                                    bindings.insert(
                                        Identifier {
                                            name: Cow::Owned(id.name.as_ref().to_owned()),
                                        },
                                        v.clone(),
                                    );
                                }

                                matcher.apply_to_env(&mut tmp_env);
                                Ok(Ok(tmp_env))
                            }
                            Err(e) => Ok(Err(e)),
                        }
                    },
                );

                match result {
                    Ok(Ok(_new_env)) => Ok(ReplOutput::Bindings(bindings)),
                    Ok(Err(_)) => Ok(ReplOutput::PatternMissmatch),
                    Err(e) => Err(e),
                }
            }
            Statement::AssignSet(mut assignments) => {
                if let Err(_e) = assignments.sort_topological(self.env.identifiers()) {
                    return Err(ReplError::AssignmentError);
                }

                let mut bindings = HashMap::new();
                let result = assignments.assignments.iter().fold(
                    Ok(Ok(self.env.clone())),
                    |acc,
                     Assignment {
                         pattern,
                         expression,
                     }| {
                        let Ok(Ok(mut tmp_env)) = acc else {
                        return acc;
                    };

                        let mut matcher = Matcher {
                            env: &tmp_env.clone(),
                            bindings: BTreeMap::new(),
                        };

                        let result = match tmp_env.eval_expr(expression) {
                            Ok(r) => r,
                            Err(_err) => {
                                return Err(ReplError::EvalError);
                            }
                        };

                        match matcher.match_pattern(pattern, &result) {
                            Ok(_) => {
                                for (id, v) in &matcher.bindings {
                                    bindings.insert(
                                        Identifier {
                                            name: Cow::Owned(id.name.as_ref().to_owned()),
                                        },
                                        v.clone(),
                                    );
                                }

                                matcher.apply_to_env(&mut tmp_env);
                                Ok(Ok(tmp_env))
                            }
                            Err(e) => Ok(Err(e)),
                        }
                    },
                );

                match result {
                    Ok(Ok(new_env)) => {
                        self.env = new_env;
                        Ok(ReplOutput::Bindings(bindings))
                    }
                    Ok(Err(_)) => Ok(ReplOutput::PatternMissmatch),
                    Err(e) => Err(e),
                }
            }
            Statement::Literal(ex) => {
                let result = match self.env.eval_expr(&ex) {
                    Ok(r) => r.to_expression(),
                    Err(_err) => {
                        return Err(ReplError::EvalError);
                    }
                };

                Ok(ReplOutput::Notice(format!("{result}")))
            }
            Statement::Pattern(pattern) => Ok(ReplOutput::Notice(format!("{pattern:?}"))),
        }
    }
}

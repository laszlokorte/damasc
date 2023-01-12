use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};

use crate::bag_bundle::BagBundle;
use crate::env::{Environment};
use crate::expression::*;
use crate::identifier::Identifier;
use crate::matcher::Matcher;
use crate::parser::{full_expression, pattern};
use crate::statement::Statement;
use crate::value::Value;

use crate::assignment::Assignment;
use crate::query::Predicate;

pub struct Repl<'i, 's, 'v> {
    pub env: Environment<'i, 's, 'v>,
    pub current_bag: Identifier<'s>,
    pub bag_bundle: BagBundle<'i, 's, 'v>,
}

impl<'i, 's, 'v> Repl<'i, 's, 'v> {
    pub fn bags(&self) -> BTreeSet<Identifier<'v>> {
        self.bag_bundle.bag_names()
    }

    pub fn vars(&self) -> BTreeSet<Identifier<'i>> {
        self.env.bindings.keys().cloned().collect()
    }
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
    Updated(usize),
    Transfered(usize),
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
            ReplOutput::Transfered(count) => writeln!(f, "MOVED {count} items."),
            ReplOutput::Updated(count) => writeln!(f, "CHANGED {count} items."),
            ReplOutput::Deleted(count) => writeln!(f, "DELETED {count} items."),
            ReplOutput::Inserted(count) => writeln!(f, "INSERTED {count} items."),
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
        let mut bag_bundle = BagBundle::new();
        
        let _ = bag_bundle.create_bag(current_bag.clone(), Predicate {
            pattern: pattern("_").unwrap().1,
            guard: full_expression("true").unwrap().1,
            limit: None,
        });

        Self {
            env,
            current_bag,
            bag_bundle,
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
                let Ok((size,guard)) = self.bag_bundle.get_bag_info(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                return Ok(ReplOutput::Notice(format!(
                    "Current Bag: {}, size: {}, constrain: {}",
                    self.current_bag,
                    size,
                    guard
                )));
            }
            Statement::ListBags => {
                return Ok(ReplOutput::Notice(format!(
                    "Bags: {}",
                    self.bag_bundle.bag_names()
                        .iter()
                        .map(|i| i.name.as_ref())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
            Statement::UseBag(bag_id, pred) => {
                self.current_bag = bag_id.clone();
                let wants_create = pred.is_some();
                match self.bag_bundle.create_bag(bag_id.clone(), Predicate {
                    pattern: pattern("_").unwrap().1,
                    guard: full_expression("true").unwrap().1,
                    limit: None,
                }) {
                    Ok(_) => {
                        return Ok(ReplOutput::Notice("BAG CREATED".into()));
                    },
                    Err(_) => {
                        if wants_create {
                            return Ok(ReplOutput::Notice("ALREADY EXISTS, SWITCHED BAG".into()));
                        } else {
                            return Ok(ReplOutput::Notice("SWITCHED BAG".into()));

                        }
                    },
                }
            }
            Statement::Import(filename) => {

                let Ok(file) = File::open(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                let lines = io::BufReader::new(file).lines();

                let result = self.bag_bundle.insert(&self.current_bag, lines.flat_map(|l| {
                    let Ok(line) = l else {
                        return Err(ReplError::ReadError);
                    };
                    let Ok((_, expr)) = full_expression(&line) else {
                        return Err(ReplError::ParseError);
                    };
                    let Ok(value) = self.env.eval_expr(&expr) else {
                        return Err(ReplError::EvalError);
                    };

                    Ok(value)
                }));

                match result {
                    Ok(count) => Ok(ReplOutput::Notice(format!(
                        "Imported {} values from file '{filename}' into current bag({})",
                        count,
                        self.current_bag
                    ))),
                    Err(_) => Err(ReplError::BagError),
                }
            }
            Statement::Export(filename) => {
                use std::io::Write;

                let Ok(file) = File::create(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                {
                    let mut file = LineWriter::new(file);
                    for v in self.bag_bundle.read(&self.current_bag).map_err(|_| ReplError::IoError)? {
                        let _ = writeln!(file, "{v}");
                    }
                }
                return Ok(ReplOutput::Notice(format!(
                    "Current bag({}) written to file: {filename}",
                    self.current_bag
                )));
            }
            Statement::Insert(expressions) => {

                let result = self.bag_bundle.insert(&self.current_bag, expressions
                    .into_iter()
                    .flat_map(|e| self.env.eval_expr(&e)));

                match result {
                    Ok(count) => {
                        if count > 0 {
                            Ok(ReplOutput::Inserted(count))
                        } else {
                            Ok(ReplOutput::No)
                        }
                    },
                    Err(_) => Err(ReplError::EvalError),
                }
            }
            Statement::Query(query) => {
                self.bag_bundle
                .query(&self.current_bag, &self.env, &query).map_err(|_| ReplError::BagError)?
                .collect::<Result<Vec<_>, _>>()
                .map(ReplOutput::Values)
                .map_err(|_| ReplError::EvalError)
            }
            Statement::Deletion(deletion) => {
                self.bag_bundle.delete(&self.current_bag, &self.env, &deletion)
                .map_err(|_| ReplError::BagError)
                .map(ReplOutput::Deleted)

            }
            Statement::Update(update) => {
                self.bag_bundle.update(&self.current_bag, &self.env, &update)
                .map_err(|_| ReplError::BagError)
                .map(ReplOutput::Updated)
            }
            Statement::Move(to, query) => {
                self.bag_bundle.transfere(&self.current_bag, &to, &self.env, query)
                .map_err(|_| ReplError::BagError)
                .map(ReplOutput::Transfered)
            },
            Statement::Pop(expression) => {
                let Ok(value) = self.env.eval_expr(&expression) else {
                    return Err(ReplError::EvalError);
                };

                match self.bag_bundle.pop(&self.current_bag, &value) {
                    Ok(false) => Ok(ReplOutput::No),
                    Ok(true) => Ok(ReplOutput::Ack),
                    Err(_) => Err(ReplError::BagError),
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

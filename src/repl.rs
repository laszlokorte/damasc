use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};

use crate::bag::{DeletionResult, InsertionResult, TransferResult, UpdateResult};
use crate::bag_bundle::BagBundle;
use crate::bag_bundle::Transaction;
use crate::env::Environment;
use crate::expression::*;
use crate::identifier::Identifier;
use crate::matcher::Matcher;
use crate::parser::{full_expression, pattern};
use crate::statement::Statement;
use crate::value::Value;

use crate::assignment::Assignment;
use crate::query::Predicate;

pub struct Repl<'b, 'i, 's, 'v> {
    pub env: Environment<'i, 's, 'v>,
    pub current_bag: Identifier<'s>,
    pub bag_bundle: BagBundle<'b, 'i, 's, 'v>,
}

impl<'b, 'i, 's, 'v> Repl<'b, 'i, 's, 'v> {
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
    Transferd(usize),
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
            ReplOutput::Transferd(count) => writeln!(f, "MOVED {count} items."),
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
    TranscationAborted,
    TransferError,
    GuardError,
}

impl<'b, 'i, 's, 'v> Repl<'b, 'i, 's, 'v> {
    pub fn new(initial_bag: &'s str) -> Self {
        let env = Environment {
            bindings: BTreeMap::new(),
        };

        let current_bag = Identifier {
            name: Cow::Borrowed(initial_bag),
        };
        let mut bag_bundle = BagBundle::new();

        let mut trans = Transaction::new(&bag_bundle);
        let _ = trans.create_bag(current_bag.clone(), Predicate::any());
        if let Ok(r) = trans.commit() {
            bag_bundle = r;
        };

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
                let mut trans = Transaction::new(&self.bag_bundle);
                let Ok((size, guard)) = trans.get_bag_info(&self.current_bag) else {
                    return Err(ReplError::BagError);
                };

                return Ok(ReplOutput::Notice(format!(
                    "Current Bag: {}, size: {}, constraint: {}",
                    self.current_bag, size, guard
                )));
            }
            Statement::ListBags => {
                let trans = Transaction::new(&self.bag_bundle);

                Ok(ReplOutput::Notice(format!(
                    "Bags: {}",
                    trans
                        .bag_names()
                        .map_err(|_| ReplError::TranscationAborted)?
                        .iter()
                        .map(|i| i.name.as_ref())
                        .collect::<Vec<_>>()
                        .join(", ")
                )))
            }
            Statement::UseBag(bag_id, pred) => {
                self.current_bag = bag_id.clone();
                let wants_create = pred.is_some();

                let mut trans = Transaction::new(&self.bag_bundle);
                let created = trans
                    .create_bag(
                        bag_id.clone(),
                        pred.unwrap_or(Predicate {
                            pattern: pattern("_").unwrap().1,
                            guard: full_expression("true").unwrap().1,
                            limit: None,
                        }),
                    )
                    .map_err(|_| ReplError::TranscationAborted)?;

                if created {
                    self.bag_bundle = trans.commit().map_err(|_| ReplError::TranscationAborted)?;
                    Ok(ReplOutput::Notice("BAG CREATED".into()))
                } else if wants_create {
                    Ok(ReplOutput::Notice("ALREADY EXISTS, SWITCHED BAG".into()))
                } else {
                    Ok(ReplOutput::Notice("SWITCHED BAG".into()))
                }
            }
            Statement::Import(filename) => {
                let Ok(file) = File::open(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                let lines = io::BufReader::new(file).lines();
                let mut trans = Transaction::new(&self.bag_bundle);
                let mut counter = 0;

                for l in lines {
                    let Ok(line) = l else {
                        return Err(ReplError::ReadError);
                    };
                    let Ok((_, expr)) = full_expression(&line) else {
                        return Err(ReplError::ParseError);
                    };

                    let result = trans
                        .insert_one(&self.current_bag, &self.env, &expr)
                        .map_err(|_| ReplError::TranscationAborted)?;
                    match result {
                        InsertionResult::Success(c) => counter += c,
                        InsertionResult::GuardError => return Err(ReplError::GuardError),
                        InsertionResult::EvalError => return Err(ReplError::EvalError),
                    }
                }

                self.bag_bundle = trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                Ok(ReplOutput::Notice(format!(
                    "Imported {} values from file '{filename}' into current bag({})",
                    counter, self.current_bag
                )))
            }
            Statement::Export(filename) => {
                use std::io::Write;

                let Ok(file) = File::create(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };

                let mut file = LineWriter::new(file);
                let trans = Transaction::new(&self.bag_bundle);
                for v in trans
                    .read(&self.current_bag)
                    .map_err(|_| ReplError::TranscationAborted)?
                {
                    let _ = writeln!(file, "{v}");
                }

                trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                return Ok(ReplOutput::Notice(format!(
                    "Current bag({}) written to file: {filename}",
                    self.current_bag
                )));
            }
            Statement::Insert(insertion) => {
                let mut trans = Transaction::new(&self.bag_bundle);
                let result = trans
                    .insert(&self.current_bag, &self.env, &insertion)
                    .map_err(|_| ReplError::TranscationAborted)?;

                match result {
                    InsertionResult::Success(count) => {
                        self.bag_bundle =
                            trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                        Ok(ReplOutput::Inserted(count))
                    }
                    InsertionResult::GuardError => Err(ReplError::GuardError),
                    InsertionResult::EvalError => Err(ReplError::EvalError),
                }
            }
            Statement::Query(query) => {
                let trans = Transaction::new(&self.bag_bundle);

                let result = trans
                    .query(&self.current_bag, &self.env, &query)
                    .map_err(|_| ReplError::TranscationAborted)?
                    .collect::<Result<Vec<_>, _>>()
                    .map(ReplOutput::Values)
                    .map_err(|_| ReplError::EvalError);

                trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                result
            }
            Statement::Deletion(deletion) => {
                let mut trans = Transaction::new(&self.bag_bundle);

                let result = trans
                    .delete(&self.current_bag, &self.env, &deletion)
                    .map_err(|_| ReplError::TranscationAborted)?;

                match result {
                    DeletionResult::Success(count) => {
                        self.bag_bundle =
                            trans.commit().map_err(|_| ReplError::TranscationAborted)?;
                        Ok(ReplOutput::Deleted(count))
                    }
                    DeletionResult::EvalError => Err(ReplError::EvalError),
                }
            }
            Statement::Update(update) => {
                let mut trans = Transaction::new(&self.bag_bundle);

                let result = trans
                    .update(&self.current_bag, &self.env, &update)
                    .map_err(|_| ReplError::TranscationAborted)?;

                match result {
                    UpdateResult::Success(count) => {
                        self.bag_bundle =
                            trans.commit().map_err(|_| ReplError::TranscationAborted)?;
                        Ok(ReplOutput::Updated(count))
                    }
                    UpdateResult::GuardError => Err(ReplError::GuardError),
                    UpdateResult::EvalError => Err(ReplError::EvalError),
                }
            }
            Statement::Move(to, query) => {
                let mut trans = Transaction::new(&self.bag_bundle);

                let result = trans
                    .transfer(&self.current_bag, &to, &self.env, query)
                    .map_err(|_| ReplError::TranscationAborted)?;

                match result {
                    TransferResult::Success(count) => {
                        self.bag_bundle =
                            trans.commit().map_err(|_| ReplError::TranscationAborted)?;
                        Ok(ReplOutput::Transferd(count))
                    }
                    TransferResult::GuardError => Err(ReplError::GuardError),
                    TransferResult::EvalError => Err(ReplError::EvalError),
                }
            }
            Statement::Pop(expression) => {
                let value = self
                    .env
                    .eval_expr(&expression)
                    .map_err(|_| ReplError::EvalError)?;

                let mut trans = Transaction::new(&self.bag_bundle);

                let result = trans
                    .pop(&self.current_bag, &value)
                    .map_err(|_| ReplError::TranscationAborted)?;

                if result {
                    self.bag_bundle = trans.commit().map_err(|_| ReplError::TranscationAborted)?;
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

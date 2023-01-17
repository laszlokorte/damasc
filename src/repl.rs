use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, BufRead, LineWriter};
use std::ops::Sub;

use crate::bag::{DeletionResult, InsertionResult, TransferResult, UpdateResult};
use crate::bag_bundle::BagBundle;
use crate::bag_bundle::Transaction;
use crate::env::Environment;
use crate::expression::*;
use crate::graph::{Graph, Connection};
use crate::graph_solver::GraphSolver;
use crate::identifier::Identifier;
use crate::matcher::Matcher;
use crate::parser::{full_expression, pattern, bundle_line, BundleCommand};
use crate::statement::Statement;
use crate::value::Value;

use crate::assignment::Assignment;
use crate::query::Predicate;

pub struct Repl<'b, 'i, 's, 'v> {
    pub env: Environment<'i, 's, 'v>,
    pub current_bag: Identifier<'s>,
    pub bag_bundle: BagBundle<'b, 'i, 's, 'v>,
    pub bag_graph: Graph<'s>,
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
pub enum ReplOutput<'x, 's, 'v> {
    Ack,
    No,
    PatternMissmatch,
    Values(Vec<Value<'s, 'v>>),
    Bindings(BTreeMap<Identifier<'x>, Value<'s, 'v>>),
    Deleted(usize),
    Inserted(usize),
    Updated(usize),
    Transferd(usize),
    Notice(String),
}

impl<'x, 's, 'v> std::fmt::Display for ReplOutput<'x, 's, 'v> {
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
    ConnectionError,
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
            bag_graph: Graph::new(),
        }
    }

    pub fn execute(&mut self, stmt: Statement<'s, 's>) -> Result<ReplOutput<'i, 's, 'v>, ReplError> {
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
            Statement::DropBag(bag_id) => {
                if self.current_bag == bag_id {
                    Err(ReplError::BagError)
                } else {
                    let mut trans = Transaction::new(&self.bag_bundle);
                    let result = trans.drop_bag(bag_id).map_err(|_| ReplError::TranscationAborted)?;

                    if result {
                        self.bag_bundle = trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                        Ok(ReplOutput::Notice("BAG REMOVED".into()))
                    } else {
                        Err(ReplError::BagError)
                    }
                }
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
            Statement::LoadBundle(filename) => {
                let Ok(file) = File::open(filename.as_ref()) else {
                    return Err(ReplError::IoError);
                };
                let lines = io::BufReader::new(file).lines();
                let mut trans = Transaction::new(&self.bag_bundle);

                let mut counter = 0;
                let mut bag_counter = 0;
                for l in lines {
                    let Ok(line) = l else {
                        return Err(ReplError::ReadError);
                    };

                    let Ok((_, cmd)) = bundle_line(&line) else {
                        return Err(ReplError::ParseError);
                    };

                    match cmd {
                        BundleCommand::Bag(bag_id, pred) => {
                            self.current_bag = bag_id.clone();
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
                                bag_counter += 1;
                            } else {
                                return Err(ReplError::BagError)
                            }
                        },
                        BundleCommand::Values(expr) => {
                            if bag_counter<1 {
                                return Err(ReplError::BagError)
                            }
                            for ex in expr.expressions {
                                let r = trans.insert_one(&self.current_bag, &self.env, &ex)
                                .map_err(|_| ReplError::TranscationAborted)?;
                                
                                match r {
                                    InsertionResult::Success(c) => {
                                        counter+= c ;
                                    },
                                    InsertionResult::GuardError => return Err(ReplError::GuardError),
                                    InsertionResult::EvalError => return Err(ReplError::EvalError),
                                }
                            }
                        },
                    }
                }                
                self.bag_bundle = trans.commit().map_err(|_| ReplError::TranscationAborted)?;

                Ok(ReplOutput::Notice(format!(
                    "Imported {} bags with {} values in total from file '{filename}' into current bag({})",
                    bag_counter, counter, self.current_bag
                )))
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
                        let mut matcher = Matcher::new(&tmp_env);

                        let result = match tmp_env.eval_expr(expression) {
                            Ok(r) => r,
                            Err(_err) => {
                                return Err(ReplError::EvalError);
                            }
                        };

                        match matcher.match_pattern(pattern, &result) {
                            Ok(_) => {
                                matcher.into_env().merge(&mut tmp_env);
                                Ok(Ok(tmp_env))
                            }
                            Err(e) => Ok(Err(e)),
                        }
                    },
                );

                match result {
                    Ok(Ok(new_env)) => Ok(ReplOutput::Bindings(new_env.bindings.clone())),
                    Ok(Err(_)) => Ok(ReplOutput::PatternMissmatch),
                    Err(e) => Err(e),
                }
            }
            Statement::AssignSet(mut assignments) => {
                if let Err(_e) = assignments.sort_topological(self.env.identifiers()) {
                    return Err(ReplError::AssignmentError);
                }

                let mut bindings = Environment::new();
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

                        let mut matcher = Matcher::new(&tmp_env);

                        let result = match tmp_env.eval_expr(expression) {
                            Ok(r) => r,
                            Err(_err) => {
                                return Err(ReplError::EvalError);
                            }
                        };

                        match matcher.match_pattern(pattern, &result) {
                            Ok(_) => {
                                matcher.local_env.clone().merge(&mut bindings);
                                matcher.local_env.clone().merge(&mut tmp_env);
                                Ok(Ok(tmp_env))
                            }
                            Err(e) => Ok(Err(e)),
                        }
                    },
                );

                match result {
                    Ok(Ok(new_env)) => {
                        self.env = new_env;
                        Ok(ReplOutput::Bindings(bindings.bindings.clone()))
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
            Statement::Connect(name, mut con) => {
                if self.bag_graph.connections.contains_key(&name) {
                    Ok(ReplOutput::Notice(format!("Connection named {name} already exists.")))
                } else {
                    match con.sort_topological(self.env.identifiers()) {
                        Ok(_) => {
                            self.bag_graph.connections.insert(name, con.clone());
                            Ok(ReplOutput::Notice(format!("Connection created:\n\n{con}"))) 
                        },
                        Err(e) => {
                            Ok(ReplOutput::Notice(format!("Topological Error in Connection, {e:?}")))   
                        },
                    }
                }
            },
            Statement::Disconnect(name) => {
                if self.bag_graph.connections.remove(&name).is_some() {
                    Ok(ReplOutput::Notice("Connection removed".into()))
                } else {
                    Err(ReplError::ConnectionError)
                }
            }
            Statement::ListConnections => {
                return Ok(ReplOutput::Notice(format!("Connections:\n\n{}\n\nUsing Bags: {:?}", self.bag_graph, self.bag_graph.bags())));
            },
            Statement::Validate => {
                let required_bags = self.bag_graph.bags();
                let existing_bags = self.bag_bundle.bag_names();
                let missing = required_bags.sub(&existing_bags);

                if missing.is_empty() {
                    Ok(ReplOutput::Notice(format!("OK")))
                } else {
                    Ok(ReplOutput::Notice(format!("Invalid, missing bags: {:?}", missing)))
                }
            },
            Statement::Solve(id) => {
                let solver = GraphSolver::new(self.env.clone(), &self.bag_bundle);
                let g = self.bag_graph.connections.clone();
                if let Some(gg) = g.get(&id) {
                    for _ in solver.solve(gg) {
                        println!("x");
                    }
                    return Ok(ReplOutput::Notice(format!("Solved")));
                } else {
                    return Ok(ReplOutput::Notice(format!("connection not defined")));
                }
            },
        }
    }
}

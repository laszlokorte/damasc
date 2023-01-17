use std::collections::BTreeMap;

use crate::{bag_bundle::BagBundle, env::Environment, graph::{Connection, Consumer, Producer}, matcher::Matcher, value::Value, identifier::Identifier};
use gen_iter::gen_iter;

pub(crate) struct GraphSolver<'bb, 'ei,'es, 'ev> {
    env: Environment<'ei,'es, 'ev>,
    bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>,
}

#[derive(Clone,Debug)]
pub(crate) struct ChangeSet<'s,'v> {
    deletions: BTreeMap<Identifier<'s>, Vec<usize>>,
    touches: BTreeMap<Identifier<'s>, Vec<usize>>,
    insertions: BTreeMap<Identifier<'s>, Vec<Value<'s, 'v>>>,
}
impl ChangeSet<'_,'_> {
    fn new() -> Self {
        Self {
            deletions: BTreeMap::new(),
            touches: BTreeMap::new(),
            insertions: BTreeMap::new(),
        }
    }
}

impl<'bb, 'ei,'es, 'ev> GraphSolver<'bb,'ei,'es, 'ev> {
    pub(crate) fn new(env: Environment<'ei,'es, 'ev>, bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>) -> Self {
        Self {
            env,
            bag_bundle,
        }
    }

    pub fn solve<'slf, 'con:'slf>(&'slf self, connection: &'con Connection<'es>)
    -> Box<dyn Iterator<Item = ChangeSet<'es, 'ev>> + 'slf> {
        let matcher = Matcher::new(&self.env);
        let changeset = ChangeSet::new();
        
        Box::new(gen_iter!(move {
            for (cc, mc) in self.solve_consumers(&connection.consumers, matcher, changeset) {
                for cp in self.solve_producers(&connection.producers, mc, cc) {
                    yield cp
                }
            }
        }))
    }


    fn solve_consumers<'slf, 'con:'slf>(&'slf self, 
    consumers: &'con [Consumer<'es>], 
    matcher: Matcher<'ei,'es, 'ev,'slf>,
    changeset: ChangeSet<'es, 'ev>) 
    -> Box<dyn Iterator<Item = (ChangeSet<'es, 'ev>, Matcher<'ei,'es, 'ev,'slf>)> + 'slf>{
        let Some(consumer) = consumers.get(0) else {
            return Box::new(Some((changeset, matcher)).into_iter())
        };
        let Some(test_bag) = self.bag_bundle.bags.get(&consumer.source_bag) else {
            return Box::new(None.into_iter());
        };
        let duplicates = Vec::with_capacity(consumer.patterns.len());
        let matcher = Matcher::new(&self.env);
        
        Box::new(gen_iter!(move {
            for m in test_bag.cross_query_helper(false, duplicates, matcher, &consumer.patterns) {
                for (cs, mm) in self.solve_consumers(&consumers[1..], m, changeset.clone()) {
                    yield (cs, mm);
                }
            }
        }))
    }

    fn solve_producers<'slf, 'con:'slf>(&'slf self, 
    producers: &'con [Producer<'es>], 
    matcher: Matcher<'ei,'es, 'ev,'slf>,
    changeset: ChangeSet<'es, 'ev>) 
    -> Box<dyn Iterator<Item = ChangeSet<'es, 'ev>> + 'slf>{
        let Some(producer) = producers.get(0) else {
            return Box::new(Some(changeset).into_iter())
        };
        let Some(target_bag) = self.bag_bundle.bags.get(&producer.target_bag) else {
            return Box::new(None.into_iter());
        };
        
        Box::new(gen_iter!(move {
            for p in &producer.projections {
                let mut env = self.env.clone();
                matcher.clone().into_env().merge(&mut env);

                match env.eval_expr(p) {
                    Ok(v) => {
                        let mut new_changeset = changeset.clone();
                        new_changeset.insertions.entry(producer.target_bag.clone()).or_insert(Vec::new()).push(v);
                        for mm in self.solve_producers(&producers[1..], matcher.clone(), new_changeset) {
                            yield mm;
                        }
                    },
                    Err(e) => {
                        dbg!(e);
                    },
                }
            }
        }))
    }
}


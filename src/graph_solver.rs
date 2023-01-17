use crate::{bag_bundle::BagBundle, env::Environment, graph::{Connection, Consumer, Producer}, matcher::Matcher, value::Value};
use gen_iter::gen_iter;

pub(crate) struct GraphSolver<'bb, 'ei,'es, 'ev> {
    env: Environment<'ei,'es, 'ev>,
    bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>,
}
impl<'bb, 'ei,'es, 'ev> GraphSolver<'bb,'ei,'es, 'ev> {
    pub(crate) fn new(env: Environment<'ei,'es, 'ev>, bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>) -> Self {
        Self {
            env,
            bag_bundle,
        }
    }

    pub fn solve<'slf, 'con:'slf>(&'slf self, connection: &'con Connection<'es>)
    -> Box<dyn Iterator<Item = Value<'es, 'ev>> + 'slf> {
        let matcher = Matcher::new(&self.env);
        
        Box::new(gen_iter!(move {
            for mc in self.solve_consumers(&connection.consumers, matcher) {
                for mp in self.solve_producers(&connection.producers, mc) {
                    yield mp
                }
            }
        }))
    }


    fn solve_consumers<'slf, 'con:'slf>(&'slf self, consumers: &'con [Consumer<'es>], matcher: Matcher<'ei,'es, 'ev,'slf>) 
    -> Box<dyn Iterator<Item = Matcher<'ei,'es, 'ev,'slf>> + 'slf>{
        let Some(consumer) = consumers.get(0) else {
            return Box::new(Some(matcher).into_iter())
        };
        let Some(test_bag) = self.bag_bundle.bags.get(&consumer.source_bag) else {
            return Box::new(None.into_iter());
        };
        let duplicates = Vec::with_capacity(consumer.patterns.len());
        let matcher = Matcher::new(&self.env);
        
        Box::new(gen_iter!(move {
            for m in test_bag.cross_query_helper(false, duplicates, matcher, &consumer.patterns) {
                for mm in self.solve_consumers(&consumers[1..], m) {
                    yield mm;
                }
            }
        }))
    }

    fn solve_producers<'slf, 'con:'slf>(&'slf self, producers: &'con [Producer<'es>], matcher: Matcher<'ei,'es, 'ev,'slf>) 
    -> Box<dyn Iterator<Item = Value<'es, 'ev>> + 'slf>{
        let Some(producer) = producers.get(0) else {
            return Box::new(Some(Value::Boolean(true)).into_iter())
        };
        let Some(target_bag) = self.bag_bundle.bags.get(&producer.target_bag) else {
            return Box::new(None.into_iter());
        };
        
        Box::new(gen_iter!(move {
            for p in &producer.projections {
                for mm in self.solve_producers(&producers[1..], matcher.clone()) {
                    yield mm;
                }
            }
        }))
    }
}


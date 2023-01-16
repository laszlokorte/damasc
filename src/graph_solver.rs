use crate::{bag_bundle::BagBundle, env::Environment, graph::{Connection, Tester, Consumer, Producer}, matcher::Matcher};
use gen_iter::gen_iter;

pub(crate) struct GraphSolver<'ee,'bb, 'ei,'es, 'ev> {
    env: &'ee Environment<'ei,'es, 'ev>,
    bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>,
}
impl<'ee,'bb:'ee, 'ei,'es, 'ev> GraphSolver<'ee,'bb,'ei,'es, 'ev> {
    pub(crate) fn new(env: &'ee Environment<'ei,'es, 'ev>, bag_bundle: &'bb BagBundle<'bb, 'ei,'es, 'ev>) -> Self {
        Self {
            env,
            bag_bundle,
        }
    }

    fn solve<'slf, 'con_s:'es, 'con:'es>(&'slf self, connection: &'con Connection<'con_s>)
    -> Box<dyn Iterator<Item = Matcher<'ei,'es, 'ev,'ee>> + 'slf> {
        let matcher = Matcher::new(self.env);
        
        Box::new(gen_iter!(move {
            for mt in self.solve_testers(&connection.testers, matcher) {
                for mc in self.solve_consumers(&connection.consumers, mt) {
                    for mp in self.solve_producers(&connection.producers, mc) {
                        yield mp
                    }
                }
            }
        }))
    }

    fn solve_testers<'slf, 'con,'con_s:'es, 'testers:'es>(&'slf self, testers: &'testers [Tester], matcher: Matcher<'ei,'es, 'ev,'ee>) 
    -> Box<dyn Iterator<Item = Matcher<'ei,'es, 'ev,'ee>> + 'slf>{
        let Some(tester) = testers.get(0) else {
            return Box::new(Some(matcher).into_iter())
        };
        let Some(test_bag) = self.bag_bundle.bags.get(&tester.test_bag) else {
            return Box::new(None.into_iter());
        };
        let duplicates = Vec::with_capacity(tester.patterns.len());
        let matcher = Matcher::new(self.env);
        
        Box::new(gen_iter!(move {
            for m in test_bag.cross_query_helper(false, duplicates, matcher, &tester.patterns) {
                for mm in self.solve_testers(&testers[1..], m) {
                    yield mm;
                }
            }
        }))
    }

    fn solve_consumers<'slf, 'con,'con_s:'es, 'consumers:'es>(&'slf self, consumers: &'consumers [Consumer], matcher: Matcher<'ei,'es, 'ev,'ee>) 
    -> Box<dyn Iterator<Item = Matcher<'ei,'es, 'ev,'ee>> + 'slf>{
        let Some(consumer) = consumers.get(0) else {
            return Box::new(Some(matcher).into_iter())
        };
        let Some(test_bag) = self.bag_bundle.bags.get(&consumer.source_bag) else {
            return Box::new(None.into_iter());
        };
        let duplicates = Vec::with_capacity(consumer.patterns.len());
        let matcher = Matcher::new(self.env);
        
        Box::new(gen_iter!(move {
            for m in test_bag.cross_query_helper(false, duplicates, matcher, &consumer.patterns) {
                for mm in self.solve_consumers(&consumers[1..], m) {
                    yield mm;
                }
            }
        }))
    }

    fn solve_producers<'slf, 'con,'con_s:'es, 'producers:'es>(&'slf self, producers: &'producers [Producer], matcher: Matcher<'ei,'es, 'ev,'ee>) 
    -> Box<dyn Iterator<Item = Matcher<'ei,'es, 'ev,'ee>> + 'slf>{
        let Some(producer) = producers.get(0) else {
            return Box::new(Some(matcher).into_iter())
        };
        let Some(test_bag) = self.bag_bundle.bags.get(&producer.target_bag) else {
            return Box::new(None.into_iter());
        };
        let matcher = Matcher::new(self.env);
        
        Box::new(gen_iter!(move {
            for p in &producer.projections {
                yield matcher.clone();
            }
        }))
    }
}


use crate::{bag_bundle::BagBundle, graph::{Connection, Consumer, Tester, Producer}, env::{Environment, EvalError}, matcher::Matcher};
use gen_iter::gen_iter;

struct GraphSolver<'b, 'e, 'i,'s,'v> {
    env: &'e Environment<'i,'s,'v>,
    bag_bundle: &'b BagBundle<'b, 'i,'s,'v>,
}

struct ConnectionBinding {

}


impl<'b:'s, 'e:'b, 'i:'s,'s,'v> GraphSolver<'b, 'e, 'i,'s,'v> {
    fn new(env: &'e Environment<'i,'s,'v>, bag_bundle: &'b BagBundle<'b, 'i,'s,'v>) -> Self {
        Self {
            env,
            bag_bundle
        }
    }

    fn solve_connection<'c:'b>(&'c self, connection: &'c Connection<'i>) -> Box<dyn Iterator<Item = ConnectionBinding> + 'e> {
        Box::new(gen_iter!(move {
            let matcher = Matcher::new(self.env);
            for t in self.solve_connection_testers(&connection.testers, matcher) {
                for c in self.solve_connection_consumers(&connection.consumers, t) {
                    for p in self.solve_connection_producers(&connection.producers, c) {
                        yield ConnectionBinding{};
                    }
                }
            }
        }))
    }

    fn solve_connection_testers<'t:'b,'m:'b>(&self, testers: &'t [Tester<'s>], matcher: Matcher<'i, 's, 'v, 'e>) -> 
    Box<dyn Iterator<Item = Matcher<'i, 's, 'v, 'e>> + 'e> {
        let Some(tester) = testers.get(0) else {
            return Box::new(Some(matcher.clone()).into_iter())
        };

        let Some(bag) = self.bag_bundle.bags.get(&tester.test_bag) else {
            return Box::new(None.into_iter());
        };
        
        
        Box::new(gen_iter!(move {
            for m in bag.cross_query_helper(
                false,
                0,
                [0; 6],
                matcher,
                &tester.patterns,
            ) {
                yield m
            }
        }))
    }

    fn solve_connection_consumers<'x:'b>(&self, consumers: &'x [Consumer<'s>], matcher: Matcher<'i, 's, 'v, 'e>) -> 
    Box<dyn Iterator<Item = Matcher<'i, 's, 'v, 'e>> + 'e> {
        let Some(consumer) = consumers.get(0) else {
            return Box::new(Some(matcher.clone()).into_iter())
        };

        let Some(bag) = self.bag_bundle.bags.get(&consumer.source_bag) else {
            return Box::new(None.into_iter());
        };
        
        
        Box::new(gen_iter!(move {
            for m in bag.cross_query_helper(
                false,
                0,
                [0; 6],
                matcher,
                &consumer.patterns,
            ) {
                yield m
            }
        }))
    }

    fn solve_connection_producers(&self, producers: &[Producer<'s>], matcher: Matcher<'i, 's, 'v, 'e>) -> 
    Box<dyn Iterator<Item = ()> + 'e>  {
        let Some(producer) = producers.get(0) else {
            return Box::new(Some(()).into_iter())
        };

        let Some(bag) = self.bag_bundle.bags.get(&producer.target_bag) else {
            return Box::new(None.into_iter());
        };
        
        
        Box::new(gen_iter!(move {
            yield ()
        }))
    }

    
}
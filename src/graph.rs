use std::collections::{BTreeSet, HashMap, HashSet};

use crate::{identifier::Identifier, expression::Expression, pattern::Pattern, assignment::AssignmentSet, literal::Literal, topology::{TopologyError, sort_topological, Node}};

#[derive(Clone)]
pub struct Graph<'s> {
    pub(crate) connections: HashMap<Identifier<'s>, Connection<'s>>
}

impl<'s> Graph<'s> {
    pub(crate) fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub(crate) fn bags(&'s self) -> BTreeSet<Identifier<'s>> {
        self.connections.values().flat_map(|con| {
            con.bags()
        }).cloned().collect()
    }
}

impl std::fmt::Display for Graph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for con in self.connections.values() {
            writeln!(f,"{con}")?;
        }
        Ok(())
    }
}

#[derive(Clone,Debug)]
pub struct Connection<'s> {
    pub(crate) signature: Signature<'s>,
    pub(crate) consumers: Vec<Consumer<'s>>,
    pub(crate) producers: Vec<Producer<'s>>,
    pub(crate) patterns: AssignmentSet<'s,'s>,
    pub(crate) guard: Expression<'s>,
}


impl<'s> Connection<'s> {

    pub(crate) fn bags(&'s self) -> impl Iterator<Item = &Identifier<'s>> {
        self.consumers.iter().map(|c| &c.source_bag).chain(
            self.producers.iter().map(|p| &p.target_bag)
        )
    }
    pub fn sort_topological<'x>(
        self,
        external_ids: HashSet<&'x Identifier>,
    ) -> Result<Connection<'s>, TopologyError<'x>> {
        let sorted = sort_topological(self.consumers, external_ids)?;
        Ok(Connection {
            consumers: sorted,
            ..self
        })
    }

}

impl std::fmt::Display for Connection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ".connection ")?;
        write!(f, "{}({})", self.signature.name, self.signature.parameter)?;
        writeln!(f,"{{")?;

        for c in &self.consumers {
            write!(f, "  &{}.{} ", c.source_bag, match c.consumption {
                Consumption::Test => "test",
                Consumption::Take => "consume",
            })?;
            for p in &c.patterns {
                write!(f, "{p};")?;
            }
        }

        for c in &self.producers {
            write!(f, "  &{}.produce ", c.target_bag)?;
            for p in &c.projections {
                write!(f, "{p};")?;
            }
            writeln!(f,";")?;
        }

        if !self.patterns.assignments.is_empty() {
            write!(f,"  let ")?;
            for p in &self.patterns.assignments {
                write!(f, "{p},")?;
            }
            writeln!(f,";")?;
        }
        if !matches!(self.guard, Expression::Literal(Literal::Boolean(true))) {
            writeln!(f,"  guard {}", self.guard)?;
        }
        writeln!(f,"}}")
    }
}


#[derive(Clone,Debug)]
pub(crate) struct Signature<'s> {
    pub(crate) name: Identifier<'s>,
    pub(crate) parameter: Pattern<'s>,
}

#[derive(Clone,Debug)]
pub(crate) enum Consumption {
    Test,
    Take,
}

#[derive(Clone,Debug)]
pub(crate) struct Consumer<'s> {
    pub(crate) consumption: Consumption,
    pub(crate) source_bag: Identifier<'s>,
    pub(crate) patterns: Vec<Pattern<'s>>,
}

impl Node for Consumer<'_> {
    type OutputIter<'x> = impl Iterator<Item = &'x Identifier<'x>> where Self: 'x;
    type InputIter<'x> = impl Iterator<Item = &'x Identifier<'x>> where Self: 'x;


    fn output_identifiers<'x>(&'x self) -> Self::OutputIter<'x> {
        self.patterns.iter().flat_map(|p| p.get_identifiers())
    }

    fn input_identifiers<'x>(&'x self) -> Self::InputIter<'x> {
        let own_output : HashSet<_> = self.output_identifiers().collect();
        self.patterns.iter().flat_map(|p| p.get_expressions()).flat_map(|e| e.get_identifiers()).filter(move |i| !own_output.contains(i))
    }
}

#[derive(Clone,Debug)]
pub(crate) struct Producer<'s> {
    pub(crate) target_bag: Identifier<'s>,
    pub(crate) projections: Vec<Expression<'s>>,
}

pub struct GraphQuery<'s> {
    pub bag: Identifier<'s>,
    pub patterns: Vec<Pattern<'s>>,
    pub guard: Expression<'s>,
}
pub struct GraphInsertion<'s> {
    pub bag: Identifier<'s>,
    pub expressions: Vec<Expression<'s>>,
}
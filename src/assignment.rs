use std::collections::{HashSet};

use crate::expression::Expression;
use crate::identifier::Identifier;
use crate::pattern::Pattern;
use crate::topology::{TopologyError, sort_topological, Node};

#[derive(Clone, Debug)]
pub struct Assignment<'a, 'b> {
    pub pattern: Pattern<'a>,
    pub expression: Expression<'b>,
}

impl std::fmt::Display for Assignment<'_,'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {};", self.pattern, self.expression)
    }
}

#[derive(Clone, Debug)]
pub struct AssignmentSet<'a, 'b> {
    pub assignments: Vec<Assignment<'a, 'b>>,
}

impl<'a, 'b> AssignmentSet<'a, 'b> {
    pub fn sort_topological<'x>(
        self,
        external_ids: HashSet<&'x Identifier>,
    ) -> Result<AssignmentSet<'a,'b>, TopologyError<'x>> {
        let sorted = sort_topological(self.assignments, external_ids)?;
        Ok(AssignmentSet {
            assignments: sorted,
        })
    }
}

impl<'a,'b> Node for Assignment<'a, 'b> {
    type OutputIter<'x> = impl Iterator<Item = &'x Identifier<'x>> where Self: 'x;
    type InputIter<'x> = impl Iterator<Item = &'x Identifier<'x>> where Self: 'x;

    fn output_identifiers<'x>(&'x self) -> Self::OutputIter<'x> {
        self.pattern.get_identifiers()
    }

    fn input_identifiers<'x>(&'x self) -> Self::InputIter<'x> {
        self.pattern.get_expressions().chain(Some(&self.expression).into_iter()).flat_map(|e| e.get_identifiers())
    }
}

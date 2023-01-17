use std::collections::{HashSet};

use crate::expression::Expression;
use crate::identifier::Identifier;
use crate::pattern::Pattern;

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

#[derive(Debug)]
pub enum AssignmentError<'s> {
    TopologicalConflict(HashSet<Identifier<'s>>),
}

impl<'s> std::fmt::Display for AssignmentError<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssignmentError::TopologicalConflict(conflicts) => {
                let _ = write!(f, "TopologicalConflict: ");
                for (n, c) in conflicts.iter().enumerate() {
                    if n > 0 {
                        let _ = write!(f, ", ");
                    }
                    let _ = write!(f, "{c}");
                }
            }
        }
        Ok(())
    }
}

impl<'a, 'b> AssignmentSet<'a, 'b> {
    pub fn sort_topological<'c>(
        &'c mut self,
        external_ids: HashSet<&Identifier>,
    ) -> Result<(), AssignmentError<'c>> {
        let mut known_ids = HashSet::new();
        let mut result: Vec<usize> = Vec::with_capacity(self.assignments.len());

        'repeat: loop {
            for (a, assignment) in self.assignments.iter().enumerate() {
                if result.contains(&a) {
                    continue;
                }

                if assignment
                    .input_identifiers()
                    .filter(|id| !external_ids.contains(id))
                    .filter(|id| !known_ids.contains(id))
                    .count()
                    == 0
                {
                    result.push(a);

                    for out_id in assignment.output_identifiers() {
                        known_ids.insert(out_id);
                    }

                    continue 'repeat;
                }
            }

            if result.len() != result.capacity() {
                let input_ids: HashSet<Identifier> = self
                    .assignments
                    .iter()
                    .flat_map(|a| a.input_identifiers())
                    .cloned()
                    .collect();
                let output_ids: HashSet<Identifier> = self
                    .assignments
                    .iter()
                    .flat_map(|a| a.output_identifiers())
                    .cloned()
                    .collect();

                let cycle: HashSet<_> = input_ids.intersection(&output_ids).cloned().collect();
                if !cycle.is_empty() {
                    return Err(AssignmentError::TopologicalConflict(cycle));
                } else {
                    return Ok(());
                }
            } else {
                self.assignments = result
                    .into_iter()
                    .map(|i| self.assignments[i].clone())
                    .collect();
                return Ok(());
            }
        }
    }
}

impl<'a, 'b> Assignment<'a, 'b> {
    fn output_identifiers(&self) -> impl Iterator<Item = &Identifier> {
        self.pattern.get_identifiers()
    }

    fn input_identifiers(&self) -> impl Iterator<Item = &Identifier> {
        self.pattern.get_expressions().chain(Some(&self.expression).into_iter()).flat_map(|e| e.get_identifiers())
    }
}

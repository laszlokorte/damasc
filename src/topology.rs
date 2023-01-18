use std::collections::HashSet;

use crate::identifier::Identifier;

pub(crate) trait Node {
    type InputIter<'s> : Iterator<Item = &'s Identifier<'s>> where Self: 's;
    type OutputIter<'s> : Iterator<Item = &'s Identifier<'s>> where Self: 's;
    fn input_identifiers<'x>(&'x self) -> Self::InputIter<'x>;
    fn output_identifiers<'x>(&'x self) -> Self::OutputIter<'x>;

}

#[derive(Debug)]
pub enum TopologyError<'s> {
    Cycle(HashSet<Identifier<'s>>),
}



impl<'s> std::fmt::Display for TopologyError<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopologyError::Cycle(conflicts) => {
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

pub(crate) fn sort_topological<'x, I: Node + Clone>(
    items: Vec<I>,
    external_ids: HashSet<&'x Identifier>,
) -> Result<Vec<I>, TopologyError<'x>> {
    let mut known_ids = HashSet::new();
    let mut result: Vec<usize> = Vec::with_capacity(items.len());

    'repeat: loop {
        for (a, assignment) in items.iter().enumerate() {
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
            let input_ids: HashSet<Identifier> = items
                .iter()
                .flat_map(|a| a.input_identifiers())
                .cloned()
                .collect();
            let output_ids: HashSet<Identifier> = items
                .iter()
                .flat_map(|a| a.output_identifiers())
                .cloned()
                .collect();

            let cycle: HashSet<_> = input_ids.intersection(&output_ids).map(|i| i.deep_clone()).collect();
            return Err(TopologyError::Cycle(cycle));
        } else {
            return Ok(result
                .into_iter()
                .map(|i| items[i].clone())
                .collect()
            );
        }
    }
}
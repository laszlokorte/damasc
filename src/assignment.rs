use std::borrow::Cow;

use crate::{pattern::Pattern, expression::Expression, identifier::Identifier};
use gen_iter::gen_iter;

#[derive(Clone,Debug)]
pub(crate) struct Assignment<'a,'b> {
    pub(crate) pattern: Pattern<'a>, 
    pub(crate) expression: Expression<'b>
}

#[derive(Clone,Debug)]
pub(crate) struct AssignmentSet<'a,'b> {
    pub(crate) assignments: Vec<Assignment<'a,'b>>
}

impl<'a,'b> AssignmentSet<'a,'b> {
    pub(crate) fn sort_topological(&mut self) {
        for assignment in &self.assignments {
            for out_id in assignment.output_identifiers() {
                for in_id in assignment.input_identifiers() {
                
                }
            }
        }
    }
}

impl<'a,'b> Assignment<'a,'b> {
    
    fn output_identifiers(&self) -> impl Iterator<Item=Identifier> {

        gen_iter!(move {
            yield Identifier{name:Cow::Borrowed("foo")};
        })
    }

    fn input_identifiers(&self) -> impl Iterator<Item=Identifier> {
        gen_iter!(move {
            yield Identifier{name:Cow::Borrowed("foo")};
        })
    }
}
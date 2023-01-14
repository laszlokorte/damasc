use std::collections::{HashSet, VecDeque};

use crate::expression::{
    ArrayItem, BinaryExpression, CallExpression, Expression, LogicalExpression, MemberExpression,
    ObjectProperty, Property, PropertyKey, StringTemplate, UnaryExpression,
};
use crate::identifier::Identifier;
use crate::pattern::{ArrayPatternItem, ObjectPropertyPattern, Pattern, PropertyPattern, Rest};

use gen_iter::gen_iter;

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
        gen_iter!(move {
            let mut stack = VecDeque::new();
            stack.push_front(&self.pattern);
            while let Some(p) = stack.pop_front() {
                match &p {
                    Pattern::Discard => {},
                    Pattern::Capture(id, _) => yield id,
                    Pattern::Identifier(id) => yield id,
                    Pattern::TypedDiscard(_) => {},
                    Pattern::TypedIdentifier(id, _) => yield id,
                    Pattern::Literal(_) => {},
                    Pattern::Object(props, rest) => {
                        for p in props {
                            match p {
                                ObjectPropertyPattern::Single(id) => yield id,
                                ObjectPropertyPattern::Match(PropertyPattern{key, value}) => {
                                    match key {
                                        PropertyKey::Identifier(id) => yield id,
                                        PropertyKey::Expression(_expr) => {},
                                    }
                                    stack.push_front(value);
                                },
                            };
                        }
                        if let Rest::Collect(p) = rest {
                            stack.push_front(p);
                        }
                    },
                    Pattern::Array(items, rest) => {
                        for ArrayPatternItem::Pattern(p) in items {
                            stack.push_front(p);
                        }
                        if let Rest::Collect(p) = rest {
                            stack.push_front(p);
                        }
                    },
                }
            }
        })
    }

    fn input_identifiers(&self) -> impl Iterator<Item = &Identifier> {
        gen_iter!(move {
            let mut expression_stack : VecDeque<&Expression> = VecDeque::new();
            let mut pattern_stack = VecDeque::new();
            pattern_stack.push_front(&self.pattern);
            while let Some(p) = pattern_stack.pop_front() {
                match &p {
                    Pattern::Discard => {},
                    Pattern::Capture(_id, _) => {},
                    Pattern::Identifier(_id) => {},
                    Pattern::TypedDiscard(_) => {},
                    Pattern::TypedIdentifier(_id, _) => {},
                    Pattern::Literal(_) => {},
                    Pattern::Object(props, rest) => {
                        for p in props {
                            match p {
                                ObjectPropertyPattern::Single(_id) => {},
                                ObjectPropertyPattern::Match(PropertyPattern{key, value}) => {
                                    match key {
                                        PropertyKey::Identifier(_id) => {},
                                        PropertyKey::Expression(expr) => expression_stack.push_front(expr),
                                    }
                                    pattern_stack.push_front(value);
                                },
                            };
                        }
                        if let Rest::Collect(p) = rest {
                            pattern_stack.push_front(p);
                        }
                    },
                    Pattern::Array(items, rest) => {
                        for ArrayPatternItem::Pattern(p) in items {
                            pattern_stack.push_front(p);
                        }
                        if let Rest::Collect(p) = rest {
                            pattern_stack.push_front(p);
                        }
                    },
                }
            };

            expression_stack.push_front(&self.expression);

            while let Some(e) = expression_stack.pop_front() {
                match e {
                    Expression::Array(arr) => {
                        for item in arr {
                            match item {
                                ArrayItem::Single(s) => {
                                    expression_stack.push_front(s);
                                },
                                ArrayItem::Spread(s) => {
                                    expression_stack.push_front(s);
                                },
                            }
                        }
                    },
                    Expression::Binary(BinaryExpression {left, right,..}) => {
                        expression_stack.push_front(left);
                        expression_stack.push_front(right);
                    },
                    Expression::Identifier(id) => yield id,
                    Expression::Literal(_) => {},
                    Expression::Logical(LogicalExpression {left, right,..}) => {
                        expression_stack.push_front(left);
                        expression_stack.push_front(right);
                    },
                    Expression::Member(MemberExpression{ object, property }) => {
                        expression_stack.push_front(object);
                        expression_stack.push_front(property);
                    },
                    Expression::Object(props) => {
                        for p in props {
                            match p {
                                ObjectProperty::Single(s) => {
                                    yield s;
                                },
                                ObjectProperty::Property(Property{key, value}) => {
                                    expression_stack.push_front(value);

                                    match key {
                                        PropertyKey::Identifier(_id) => {},
                                        PropertyKey::Expression(expr) =>
                                        expression_stack.push_front(expr),
                                    };
                                },
                                ObjectProperty::Spread(s) => {
                                    expression_stack.push_front(s);
                                },
                            }
                        }
                    },
                    Expression::Unary(UnaryExpression{argument, ..}) => {
                        expression_stack.push_front(argument);
                    },
                    Expression::Call(CallExpression{argument,..}) => {
                        expression_stack.push_front(argument);

                    },
                    Expression::Template(StringTemplate{parts, ..}) => {
                        for p in parts {
                            expression_stack.push_front(&p.dynamic_end);
                        }
                    },
                }
            }
        })
    }
}

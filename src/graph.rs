use crate::{identifier::Identifier, expression::Expression, pattern::Pattern, assignment::AssignmentSet, literal::Literal};

pub struct Graph<'s> {
    pub(crate) connections: Vec<Connection<'s>>
}

impl<'s> Graph<'s> {
    pub(crate) fn new() -> Self {
        Self {
            connections: vec![],
        }
    }
}

impl std::fmt::Display for Graph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for con in &self.connections {
            writeln!(f,"{con}")?;
        }
        Ok(())
    }
}

#[derive(Clone,Debug)]
pub struct Connection<'s> {
    pub(crate) signature: Option<Signature<'s>>,
    pub(crate) consumers: Vec<Consumer<'s>>,
    pub(crate) producers: Vec<Producer<'s>>,
    pub(crate) testers: Vec<Tester<'s>>,
    pub(crate) patterns: AssignmentSet<'s,'s>,
    pub(crate) guard: Expression<'s>,
}

impl std::fmt::Display for Connection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ".connection ")?;
        if let Some(sig) = &self.signature {
            write!(f, "{}({})", sig.name, sig.parameter)?;
        }
        writeln!(f,"{{")?;

        for c in &self.testers {
            write!(f, "  &{}.test ", c.test_bag)?;
            for p in &c.patterns {
                write!(f, "{p};")?;
            }
            write!(f, " where {}", c.guard)?;
            writeln!(f,";")?;
        }

        for c in &self.consumers {
            write!(f, "  &{}.consume ", c.source_bag)?;
            for p in &c.patterns {
                write!(f, "{p};")?;
            }
            write!(f, " where {}", c.guard)?;
            writeln!(f,";")?;
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
pub(crate) struct Consumer<'s> {
    pub(crate) source_bag: Identifier<'s>,
    pub(crate) patterns: Vec<Pattern<'s>>,
    pub(crate) guard: Expression<'s>,
}

#[derive(Clone,Debug)]
pub(crate) struct Producer<'s> {
    pub(crate) target_bag: Identifier<'s>,
    pub(crate) projections: Vec<Expression<'s>>,
}

#[derive(Clone,Debug)]
pub(crate) struct Tester<'s> {
    pub(crate) test_bag: Identifier<'s>,
    pub(crate) patterns: Vec<Pattern<'s>>,
    pub(crate) guard: Expression<'s>,
}
use std::collections::VecDeque;

use crate::expression::{PropertyKey, Expression};
use crate::identifier::Identifier;
use crate::literal::Literal;
use crate::value::ValueType;

use gen_iter::gen_iter;


#[derive(Clone, Debug)]
pub enum Pattern<'s> {
    Discard,
    Capture(Identifier<'s>, Box<Pattern<'s>>),
    Identifier(Identifier<'s>),
    TypedDiscard(ValueType),
    TypedIdentifier(Identifier<'s>, ValueType),
    Literal(Literal<'s>),
    Object(ObjectPattern<'s>, Rest<'s>),
    Array(ArrayPattern<'s>, Rest<'s>),
}

impl<'a> std::fmt::Display for Pattern<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Pattern::Discard => write!(f, "_"),
            Pattern::Literal(l) => write!(f, "{l}"),
            Pattern::Capture(id, pat) => write!(f, "{pat} @ {id}"),
            Pattern::TypedDiscard(t) => write!(f, "_ is {t}"),
            Pattern::Identifier(id) => write!(f, "{id}"),
            Pattern::TypedIdentifier(id, t) => write!(f, "{id} is {t}"),
            Pattern::Object(props, rest) => {
                let _ = write!(f, "{{");

                for prop in props {
                    let _ = match prop {
                        ObjectPropertyPattern::Single(p) => write!(f, "{p}"),
                        ObjectPropertyPattern::Match(PropertyPattern { key, value }) => {
                            let _ = match key {
                                PropertyKey::Identifier(id) => {
                                    write!(f, "{id}")
                                }
                                PropertyKey::Expression(e) => {
                                    write!(f, "{e}")
                                }
                            };

                            write!(f, ": {value}")
                        }
                    };
                    let _ = write!(f, ",");
                }

                match rest {
                    Rest::Exact => {}
                    Rest::Discard => {
                        let _ = write!(f, "...");
                    }
                    Rest::Collect(p) => {
                        let _ = write!(f, "...{p}");
                    }
                };

                write!(f, "}}")
            }
            Pattern::Array(items, rest) => {
                let _ = write!(f, "[");
                for ArrayPatternItem::Pattern(item) in items {
                    let _ = write!(f, "{item},");
                }

                match rest {
                    Rest::Exact => {}
                    Rest::Discard => {
                        let _ = write!(f, "...");
                    }
                    Rest::Collect(p) => {
                        let _ = write!(f, "...{p}");
                    }
                };
                write!(f, "]")
            }
        };
        write!(f, "")
    }
}

impl Pattern<'_> {
    pub(crate) fn get_identifiers(&self) -> impl Iterator<Item = &Identifier> {
        gen_iter!(move {
            let mut stack = VecDeque::new();
            stack.push_front(self);
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

    pub(crate) fn get_expressions(&self) -> impl Iterator<Item = &Expression> {
        gen_iter!(move {
            let mut pattern_stack = VecDeque::new();
            pattern_stack.push_front(self);
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
                                        PropertyKey::Expression(expr) => {
                                            yield expr;
                                        },
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
        })
    }
}

#[derive(Clone, Debug)]
pub enum Rest<'s> {
    Exact,
    Discard,
    Collect(Box<Pattern<'s>>),
}

pub type ObjectPattern<'a> = Vec<ObjectPropertyPattern<'a>>;
pub type ArrayPattern<'a> = Vec<ArrayPatternItem<'a>>;

#[derive(Clone, Debug)]
pub enum ArrayPatternItem<'a> {
    Pattern(Pattern<'a>),
    //Expression(Expression<'a>),
}

#[derive(Clone, Debug)]
pub enum ObjectPropertyPattern<'a> {
    Single(Identifier<'a>),
    Match(PropertyPattern<'a>),
}

#[derive(Clone, Debug)]
pub struct PropertyPattern<'a> {
    pub key: PropertyKey<'a>,
    pub value: Pattern<'a>,
}

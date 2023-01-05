use std::borrow::Cow;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct Identifier<'a> {
    pub(crate) name: Cow<'a, str>,
}


impl std::fmt::Display for Identifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
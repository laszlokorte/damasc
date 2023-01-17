use std::borrow::Cow;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Identifier<'a> {
    pub name: Cow<'a, str>,
}

impl std::fmt::Display for Identifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Identifier<'_> {
    pub(crate) fn deep_clone<'x,'y>(&'x self) -> Identifier<'y> {
        Identifier { name: Cow::Owned(self.name.as_ref().into()) }
    }
}
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Value<'s, 'v> {
    Null,
    String(Cow<'s, str>),
    Integer(i64),
    Boolean(bool),
    Array(Vec<Cow<'v, Value<'s, 'v>>>),
    Object(ValueObjectMap<'s, 'v>),
    Type(ValueType),
}

pub(crate) type ValueObjectMap<'s, 'v> = BTreeMap<Cow<'s, str>, Cow<'v, Value<'s, 'v>>>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum ValueType {
    Null,
    String,
    Integer,
    Boolean,
    Array,
    Object,
    Type,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl<'s, 'v> Value<'s, 'v> {
    pub(crate) fn get_type(&self) -> ValueType {
        match self {
            Value::Null => ValueType::Null,
            Value::String(_) => ValueType::String,
            Value::Integer(_) => ValueType::Integer,
            Value::Boolean(_) => ValueType::Boolean,
            Value::Array(_) => ValueType::Array,
            Value::Object(_) => ValueType::Object,
            Value::Type(_) => ValueType::Type,
        }
    }
}

impl<'s, 'v> std::fmt::Display for Value<'s, 'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = match self {
            Value::Null => write!(f, "null"),
            Value::String(s) => write!(f, "\"{s}\""),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Array(a) => {
                let _ = write!(f, "[");
                for v in a {
                    let _ = write!(f, "{v}, ",);
                }
                write!(f, "]")
            }
            Value::Object(o) => {
                let _ = write!(f, "{{");
                for (k, v) in o {
                    let _ = write!(f, "{k}: ",);
                    let _ = write!(f, "{v}, ",);
                }
                write!(f, "}}")
            }
            Value::Type(t) => write!(f, "{t}"),
        };
        write!(f, "")
    }
}

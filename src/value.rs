use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Value<'s, 'v> {
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
pub enum ValueType {
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

    pub(crate) fn convert(&self, specified_type: ValueType) -> Option<Value<'s, 'v>> {
        if self.get_type() == specified_type {
            return Some(self.clone());
        }

        Some(match (&self, specified_type) {
            (Value::Null, ValueType::String) => Value::String(Cow::Borrowed("null")),
            (Value::Null, ValueType::Integer) => Value::Integer(0),
            (Value::Null, ValueType::Boolean) => Value::Boolean(false),
            (Value::Null, ValueType::Array) => Value::Array(vec![]),
            (Value::Null, ValueType::Object) => Value::Object(BTreeMap::new()),
            (_, ValueType::Type) => Value::Type(self.get_type()),
            (Value::Type(t), ValueType::String) => Value::String(Cow::Owned(format!("{t}"))),
            (Value::Object(o), ValueType::Array) => Value::Array(o.values().cloned().collect()),
            (Value::Object(o), ValueType::Boolean) => Value::Boolean(!o.is_empty()),
            (Value::Array(a), ValueType::Boolean) => Value::Boolean(!a.is_empty()),
            (Value::String(s), ValueType::Boolean) => Value::Boolean(!s.is_empty()),
            (Value::String(s), ValueType::Array) => Value::Array(
                s.chars()
                    .map(|c| Cow::Owned(Value::String(Cow::Owned(c.to_string()))))
                    .collect(),
            ),
            (Value::String(_), ValueType::Object) => todo!(),
            (Value::Integer(i), ValueType::String) => Value::String(Cow::Owned(i.to_string())),
            (Value::Integer(i), ValueType::Boolean) => Value::Boolean(i != &0),
            (Value::Boolean(b), ValueType::String) => Value::String(Cow::Owned(b.to_string())),
            (Value::Boolean(b), ValueType::Integer) => Value::Integer(if *b { 1 } else { 0 }),
            (Value::Array(a), ValueType::Integer) => Value::Integer(a.len() as i64),
            (Value::Object(o), ValueType::Integer) => Value::Integer(o.len() as i64),
            _ => return None,
        })
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

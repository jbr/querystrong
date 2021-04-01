use crate::{IndexPath, Indexer, Result};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use std::mem;
use std::{collections::BTreeMap, fmt::Debug, iter, ops::Index};

#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    Map(BTreeMap<String, Value>),
    List(Vec<Value>),
    String(String),
    Empty,
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Map(m) => f.debug_map().entries(m).finish(),
            Value::List(l) => f.debug_list().entries(l).finish(),
            Value::String(s) => Debug::fmt(s, f),
            _ => f.write_str("()"),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Empty
    }
}

impl Value {
    pub fn new_map() -> Self {
        Self::Map(BTreeMap::new())
    }

    pub fn new_list() -> Self {
        Self::List(Vec::new())
    }

    pub fn is_map(&self) -> bool {
        matches!(self, &Self::Map(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, &Self::String(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, &Self::List(_))
    }

    pub fn as_slice(&self) -> Option<&[Self]> {
        match self {
            Self::List(ref l) => Some(&l[..]),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Map(ref m) => Some(&m),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(ref s) => Some(&**s),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Value::Map(m) => m.is_empty(),
            Value::List(l) => l.is_empty() || l.iter().all(Value::is_empty),
            Value::String(s) => s.is_empty(),
            Value::Empty => true,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Value::Map(m) => m.len(),
            Value::List(l) => l.len(),
            Value::String(s) => s.len(),
            Value::Empty => 0,
        }
    }

    pub fn append(&mut self, key: impl Into<IndexPath>, value: impl Into<Value>) -> Result<()> {
        let mut index_path = key.into();
        *self = mem::take(self).inner_append(index_path.pop_front(), index_path, value.into())?;
        Ok(())
    }

    pub fn get(&self, key: impl Into<IndexPath>) -> Option<&Value> {
        let mut index_path = key.into();
        let key = index_path.pop_front();
        match (self, key) {
            (Value::Map(m), Some(Indexer::String(key))) => {
                m.get(&*key).and_then(|v| v.get(index_path))
            }

            (Value::List(l), Some(Indexer::Number(key))) => {
                l.get(key).and_then(|v| v.get(index_path))
            }

            (this, None) => Some(this),

            _ => None,
        }
    }

    pub fn get_str(&self, key: impl Into<IndexPath>) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_slice(&self, key: impl Into<IndexPath>) -> Option<&[Value]> {
        self.get(key).and_then(Value::as_slice)
    }

    pub fn get_map(&self, key: impl Into<IndexPath>) -> Option<&BTreeMap<String, Value>> {
        self.get(key).and_then(Value::as_map)
    }

    fn inner_append(
        self,
        current_index: Option<Indexer>,
        index_path: IndexPath,
        value: Value,
    ) -> Result<Self> {
        match (self, current_index, value) {
            (Value::Map(mut m), Some(Indexer::String(key)), value) => {
                m.entry(key).or_default().append(index_path, value)?;
                Ok(Value::Map(m))
            }

            (Value::Empty, None, value) => Ok(value),
            (Value::Empty, Some(Indexer::Empty), value) => Ok(Value::List(vec![value])),

            (Value::Empty, Some(Indexer::String(s)), Value::Empty) => Ok(Value::String(s)),
            (Value::Empty, Some(Indexer::String(s)), value) => Value::Map(BTreeMap::new())
                .inner_append(Some(Indexer::String(s)), index_path, value),

            (Value::String(ref s), None, value)
            | (Value::String(ref s), Some(Indexer::Empty), value) => {
                Ok(Self::List(vec![s.into(), value]))
            }

            (Value::String(s1), Some(Indexer::String(s2)), Value::Empty) => {
                Ok(Self::List(vec![Value::String(s1), Value::String(s2)]))
            }

            (Value::List(mut l), Some(Indexer::Empty), value) => {
                l.push(value);
                Ok(Value::List(l))
            }

            (Value::List(mut l), None, value) => {
                l.push(value);
                Ok(Value::List(l))
            }

            (Value::List(mut l), Some(Indexer::Number(n)), value) => {
                while l.len() <= n {
                    l.push(Value::Empty);
                }

                *l.get_mut(n).unwrap() = value;
                Ok(Value::List(l))
            }

            (Value::List(mut l), Some(Indexer::String(s)), Value::Empty) => {
                l.push(s.into());
                Ok(Value::List(l))
            }

            (Value::List(l), Some(Indexer::String(s)), value) => {
                let mut map = BTreeMap::new();
                for v in l {
                    match v {
                        Value::String(s) => {
                            map.insert(s, Value::Empty);
                        }
                        _ => return Err(format!("could not convert {:?} to a map", v).into()),
                    }
                }
                map.insert(s, value);
                Ok(Value::Map(map))
            }

            (current_value, _, Value::Empty) => Ok(current_value),

            (Value::Empty, current_index @ Some(Indexer::Number(_)), value) => {
                Value::List(vec![]).inner_append(current_index, index_path, value)
            }

            (previous_value, indexer, new_value) => {
                return Err(format!(
                    "could not append ({:?}, {:?}, {:?})",
                    previous_value, indexer, new_value
                )
                .into());
            }
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Self::from(&s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(percent_decode_str(s).decode_utf8_lossy().into())
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Self::from(&**s)
    }
}

impl<V> From<Option<V>> for Value
where
    V: Into<Value>,
{
    fn from(o: Option<V>) -> Self {
        match o {
            Some(o) => o.into(),
            None => Value::Empty,
        }
    }
}

impl<K, V> From<(K, V)> for Value
where
    K: Into<IndexPath>,
    V: Into<Value>,
{
    fn from(v: (K, V)) -> Self {
        let (key, value) = v;
        let mut v = Value::Empty;
        v.append(key, value).unwrap();
        v
    }
}

impl<I: Into<Value>> From<Vec<I>> for Value {
    fn from(v: Vec<I>) -> Self {
        Value::List(v.into_iter().map(|v| v.into()).collect())
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

impl PartialEq<()> for Value {
    fn eq(&self, _: &()) -> bool {
        self == &Self::Empty
    }
}

impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        match self {
            Value::String(ref s) => s == other,
            _ => false,
        }
    }
}

impl PartialEq<str> for Value {
    fn eq(&self, other: &str) -> bool {
        match self {
            Value::String(ref s) => s == other,
            _ => false,
        }
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Value::String(s) => s == other,
            _ => false,
        }
    }
}

impl<'a> IntoIterator for &'a Value {
    type Item = (Option<IndexPath>, Option<String>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Value::Map(m) => Box::new(m.iter().flat_map(|(k1, v)| {
                v.into_iter().map(move |(k2, v)| match k2 {
                    Some(mut k2) => {
                        k2.push_front(Indexer::from(k1));
                        (Some(k2), v)
                    }
                    None => (Some(Indexer::from(k1).into()), v),
                })
            })),

            Value::List(l) => Box::new(l.iter().flat_map(|v| {
                v.into_iter().map(move |(k, v)| match k {
                    Some(mut k) => {
                        k.push_front(().into());
                        (Some(k), v)
                    }
                    None => (Some(().into()), v),
                })
            })),

            Value::String(s) => Box::new(iter::once((
                None,
                Some(utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()),
            ))),

            Value::Empty => Box::new(iter::once((None, None))),
        }
    }
}

impl<Key: Into<IndexPath>> Index<Key> for Value {
    type Output = Self;

    fn index(&self, key: Key) -> &Self::Output {
        self.get(key).unwrap()
    }
}

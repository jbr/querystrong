use crate::{Error, IndexPath, Indexer};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::Index;

#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    Map(BTreeMap<String, Box<Value>>),
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
            Value::Empty => f.write_str("()"),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Empty
    }
}

impl Value {
    fn inner_append(
        self,
        current_index: Option<Indexer>,
        index_path: IndexPath,
        value: Value,
    ) -> Result<Self, Error> {
        match (self, current_index) {
            (Value::Map(mut m), Some(Indexer::String(key))) => {
                m.entry(key).or_default().append(index_path, value)?;
                Ok(Value::Map(m))
            }

            (Value::Empty, None) => Ok(value),

            (Value::String(ref s), None) => Ok(Self::List(vec![s.into(), value])),

            (Value::List(mut l), Some(Indexer::Empty)) => {
                l.push(value);
                Ok(Value::List(l))
            }

            (Value::List(mut l), Some(Indexer::Number(n))) => {
                while l.len() <= n {
                    l.push(Value::Empty);
                }

                *l.get_mut(n).unwrap() = value;
                Ok(Value::List(l))
            }

            (Value::Empty, current_index @ Some(Indexer::String(_))) => {
                Value::Map(BTreeMap::new()).inner_append(current_index, index_path, value)
            }

            (Value::Empty, current_index @ Some(Indexer::Number(_))) => {
                Value::List(vec![]).inner_append(current_index, index_path, value)
            }

            (Value::Empty, current_index @ Some(Indexer::Empty)) => {
                Value::List(vec![]).inner_append(current_index, index_path, value)
            }

            (v, indexer) => {
                return Err(Error::Stuff(format!(
                    "could not append with {:?} to {:?}",
                    indexer, v
                )))
            }
        }
    }

    pub fn append(
        &mut self,
        key: impl Into<IndexPath>,
        value: impl Into<Value>,
    ) -> Result<(), Error> {
        let mut index_path = key.into();
        *self =
            std::mem::take(self).inner_append(index_path.pop_front(), index_path, value.into())?;
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
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(String::from(s))
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Value::String(String::from(s))
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

// impl From<QueryStrong> for Value {
//     fn from(q: QueryStrong) -> Self {
//         q.root
//     }
// }

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

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Value::String(s) => s == other,
            _ => false,
        }
    }
}

impl<'a> IntoIterator for &'a Value {
    type Item = (Option<IndexPath>, Option<&'a str>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Value::Map(m) => Box::new(m.iter().flat_map(|(k1, v)| {
                v.into_iter().map(move |(k2, v)| match k2 {
                    Some(mut k2) => {
                        k2.insert(0, k1.into());
                        (Some(k2), v)
                    }
                    None => (Some(k1.into()), v),
                })
            })),

            Value::List(l) => Box::new(l.iter().flat_map(|v| {
                v.into_iter().map(move |(k, v)| match k {
                    Some(mut k) => {
                        k.insert(0, ().into());
                        (Some(k), v)
                    }
                    None => (Some(().into()), v),
                })
            })),
            Value::String(s) => Box::new(std::iter::once((None, Some(&**s)))),
            Value::Empty => Box::new(std::iter::once((None, None))),
        }
    }
}

impl<K> Index<K> for Value
where
    K: Into<IndexPath>,
{
    type Output = Self;

    fn index(&self, k: K) -> &Self::Output {
        self.get(k).unwrap()
    }
}

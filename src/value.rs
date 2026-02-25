use crate::{Error, IndexPath, Indexer, Result};
use std::borrow::Cow;
use std::convert::TryInto;
use std::mem;
use std::{collections::BTreeMap, fmt::Debug, iter, ops::Index};

/// A node in the parsed query-string value tree.
///
/// The lifetime `'a` reflects zero-copy borrowing: string data that needs no
/// percent-decoding or `+`-as-space substitution is stored as a `Cow::Borrowed`
/// pointing into the original input.  Call [`Value::into_owned`] to obtain a
/// `'static` value.
///
/// # Variant summary
///
/// | Variant        | Produced by                    | Serializes as   |
/// |----------------|--------------------------------|-----------------|
/// | `Map`          | string-keyed brackets          | `k[sub]=v`      |
/// | `List`         | `[]` appends                   | `k[]=v`         |
/// | `SparseList`   | explicit `[n]` numeric indices | `k[n]=v`        |
/// | `String`       | plain value (`k=v`)            | `k=v`           |
/// | `Empty`        | key with no value (`k`)        | `k`             |
#[derive(Clone, PartialEq, Eq, Default)]
pub enum Value<'a> {
    /// A string-keyed map, produced by bracket-notation keys (`a[b]=v`).
    Map(BTreeMap<Cow<'a, str>, Value<'a>>),
    /// A dense, contiguous list produced by empty-bracket appends (`a[]=v`).
    ///
    /// Serializes with `[]` notation.
    List(Vec<Value<'a>>),
    /// A sparse list produced by explicit numeric indices (`a[5]=v`).
    ///
    /// Backed by a `BTreeMap<usize, Value>` so a large index like `a[999999]=v`
    /// allocates exactly one map slot rather than 1 000 000 `Vec` elements.
    /// Absent slots within `0..=max_key` are treated as [`Value::Empty`];
    /// slots beyond `max_key` return `None` from [`Value::get`].
    ///
    /// Serializes with `[n]` notation, preserving the original indices.
    SparseList(BTreeMap<usize, Value<'a>>),
    /// A string value, possibly borrowed from the input when no decoding was needed.
    String(Cow<'a, str>),
    /// The absence of a value, produced by a key with no `=` (e.g. bare `k`).
    #[default]
    Empty,
}

impl Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Map(m) => f.debug_map().entries(m).finish(),
            Value::List(l) => f.debug_list().entries(l).finish(),
            Value::SparseList(m) => f.debug_map().entries(m).finish(),
            Value::String(s) => Debug::fmt(s, f),
            _ => f.write_str("()"),
        }
    }
}

impl<'a> Value<'a> {
    /// Deep-clone this value into a `Value<'static>`, converting any
    /// `Cow::Borrowed` strings to owned `String`s.
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Value::Map(btree_map) => Value::Map(
                btree_map
                    .into_iter()
                    .map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned()))
                    .collect(),
            ),
            Value::List(values) => {
                Value::List(values.into_iter().map(|v| v.into_owned()).collect())
            }
            Value::SparseList(btree_map) => Value::SparseList(
                btree_map
                    .into_iter()
                    .map(|(k, v)| (k, v.into_owned()))
                    .collect(),
            ),
            Value::String(cow) => Value::String(cow.into_owned().into()),
            Value::Empty => Value::Empty,
        }
    }

    /// Builds a querystrong::Value::Map
    pub fn new_map() -> Self {
        Self::Map(BTreeMap::new())
    }

    /// Builds a dense querystrong::Value::List (for `[]` appends)
    pub fn new_list() -> Self {
        Self::List(Vec::new())
    }

    /// Builds a querystrong::Value::SparseList (for `[n]` indexed access)
    pub fn new_sparse_list() -> Self {
        Self::SparseList(BTreeMap::new())
    }

    /// Returns `true` if this value is a [`Map`](Value::Map).
    pub fn is_map(&self) -> bool {
        matches!(self, &Self::Map(_))
    }

    /// Returns `true` if this value is a [`String`](Value::String).
    pub fn is_string(&self) -> bool {
        matches!(self, &Self::String(_))
    }

    /// Returns true for both dense `List` and `SparseList` variants.
    pub fn is_list(&self) -> bool {
        matches!(self, &Self::List(_) | &Self::SparseList(_))
    }

    /// Returns true only for the dense `List` variant (built from `[]` appends).
    pub fn is_dense_list(&self) -> bool {
        matches!(self, &Self::List(_))
    }

    /// Returns true only for the `SparseList` variant (built from `[n]` indices).
    pub fn is_sparse_list(&self) -> bool {
        matches!(self, &Self::SparseList(_))
    }

    /// Returns a slice only for the dense `List` variant; returns `None` for `SparseList`.
    pub fn as_slice(&self) -> Option<&[Self]> {
        match self {
            Self::List(l) => Some(&l[..]),
            _ => None,
        }
    }

    /// Returns a reference to the inner map if this is a [`Map`](Value::Map),
    /// otherwise `None`.
    pub fn as_map(&self) -> Option<&BTreeMap<Cow<'a, str>, Value<'a>>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Returns a reference to the inner map if this is a [`SparseList`](Value::SparseList),
    /// otherwise `None`.
    pub fn as_sparse_list(&self) -> Option<&BTreeMap<usize, Value<'a>>> {
        match self {
            Self::SparseList(m) => Some(m),
            _ => None,
        }
    }

    /// Returns the inner string slice if this is a [`String`](Value::String),
    /// otherwise `None`.
    ///
    /// When the original input was not percent-encoded, the returned `&str`
    /// points directly into the source `&'a str` without copying.
    pub fn as_str<'b: 'a>(&'b self) -> Option<&'a str> {
        match self {
            Self::String(s) => Some(&**s),
            _ => None,
        }
    }

    /// this is a general-purpose predicate that is broader than just
    /// whether the value is an Empty. Zero-length Values of any sort
    /// will return true from is_empty
    pub fn is_empty(&self) -> bool {
        match self {
            Value::Map(m) => m.is_empty(),
            Value::List(l) => l.is_empty() || l.iter().all(Value::is_empty),
            Value::SparseList(m) => m.is_empty() || m.values().all(Value::is_empty),
            Value::String(s) => s.is_empty(),
            Value::Empty => true,
        }
    }

    /// For `List`, returns the number of elements.
    /// For `SparseList`, returns the number of populated entries (not `last_index + 1`).
    pub fn len(&self) -> usize {
        match self {
            Value::Map(m) => m.len(),
            Value::List(l) => l.len(),
            Value::SparseList(m) => m.len(),
            Value::String(s) => s.len(),
            Value::Empty => 0,
        }
    }

    /// Insert or merge `value` at the path described by `key`.
    ///
    /// `key` accepts anything convertible to an [`IndexPath`]: a `&str` like
    /// `"a[b][c]"`, a bare `usize`, an [`Indexer`], or a pre-built `IndexPath`.
    /// `value` accepts anything convertible to a [`Value`]: `&str`, `String`,
    /// `Option<V>`, `()`, `Vec<V>`, or a `(key, value)` pair.
    ///
    /// Returns an error when the existing tree structure is incompatible with
    /// the requested path (e.g. appending `a[b]=2` when `a` is already a
    /// string).
    pub fn append<'b: 'a, K, V>(&mut self, key: K, value: V) -> Result<'a, ()>
    where
        K: TryInto<IndexPath<'b>>,
        V: TryInto<Value<'b>>,
        K::Error: Into<Error<'a>>,
        V::Error: Into<Error<'a>>,
    {
        let mut index_path = key.try_into().map_err(Into::into)?;
        let value = value.try_into().map_err(Into::into)?;
        let (self_value, error) =
            mem::take(self).inner_append(index_path.pop_front(), index_path, value);
        *self = self_value;
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Traverse the value tree along `key`, returning the node at that path.
    ///
    /// Returns `None` if any segment of the path does not match the current
    /// value's type or if the key is absent.
    ///
    /// For a [`SparseList`](Value::SparseList), indices within `0..=max_index`
    /// that have no stored value return `Some(&Value::Empty)`; indices beyond
    /// `max_index` return `None`.
    pub fn get<'b>(&self, key: impl TryInto<IndexPath<'b>>) -> Option<&Value<'a>> {
        let mut index_path = key.try_into().ok()?;
        let key = index_path.pop_front();
        match (self, key) {
            (Value::Map(m), Some(Indexer::String(key))) => {
                m.get(&*key).and_then(|v| v.get(index_path))
            }

            (Value::List(l), Some(Indexer::Number(key))) => {
                l.get(key).and_then(|v| v.get(index_path))
            }

            (Value::SparseList(m), Some(Indexer::Number(key))) => {
                // Absent slots within 0..=max_key behave like Value::Empty (consistent
                // with dense List), so q["a"][0] doesn't panic for a[2]=v.
                // Uses a static to return a reference without any allocation.
                static EMPTY: Value<'static> = Value::Empty;
                let within_range = m.last_key_value().is_some_and(|(&max, _)| key <= max);
                let v: &Value<'a> = match m.get(&key) {
                    Some(v) => v,
                    None if within_range => &EMPTY,
                    None => return None,
                };
                v.get(index_path)
            }

            (this, None) => Some(this),

            _ => None,
        }
    }

    /// Convenience wrapper around [`get`](Value::get) that extracts a `&str`.
    ///
    /// Equivalent to `self.get(key).and_then(Value::as_str)`.
    pub fn get_str<'b>(&self, key: impl TryInto<IndexPath<'b>>) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Convenience wrapper around [`get`](Value::get) that extracts a `&[Value]`.
    ///
    /// Only succeeds for dense [`List`](Value::List) values; returns `None` for
    /// [`SparseList`](Value::SparseList).  Equivalent to
    /// `self.get(key).and_then(Value::as_slice)`.
    pub fn get_slice<'b>(&self, key: impl TryInto<IndexPath<'b>>) -> Option<&[Value<'a>]> {
        self.get(key).and_then(Value::as_slice)
    }

    /// Convenience wrapper around [`get`](Value::get) that extracts the inner map.
    ///
    /// Equivalent to `self.get(key).and_then(Value::as_map)`.
    pub fn get_map<'b>(
        &self,
        key: impl TryInto<IndexPath<'b>>,
    ) -> Option<&BTreeMap<Cow<'a, str>, Value<'a>>> {
        self.get(key).and_then(Value::as_map)
    }

    /// Convenience wrapper around [`get`](Value::get) that extracts the sparse-list map.
    ///
    /// Equivalent to `self.get(key).and_then(Value::as_sparse_list)`.
    pub fn get_sparse_list<'b>(
        &self,
        key: impl TryInto<IndexPath<'b>>,
    ) -> Option<&BTreeMap<usize, Value<'a>>> {
        self.get(key).and_then(Value::as_sparse_list)
    }

    fn inner_append<'b: 'a>(
        self,
        current_index: Option<Indexer<'b>>,
        index_path: IndexPath<'b>,
        value: Value<'b>,
    ) -> (Self, Option<Error<'a>>) {
        match (self, current_index, value) {
            (Value::Map(mut m), Some(Indexer::String(key)), value) => {
                let err = m.entry(key).or_default().append(index_path, value).err();
                (Value::Map(m), err)
            }

            (Value::Empty, None, value) => (value, None),
            (Value::Empty, Some(Indexer::Empty), value) => (Value::List(vec![value]), None),

            (Value::Empty, Some(Indexer::String(s)), Value::Empty) => (Value::String(s), None),
            (Value::Empty, Some(Indexer::String(s)), value) => Value::Map(BTreeMap::new())
                .inner_append(Some(Indexer::String(s)), index_path, value),

            (Value::String(s), None, value) | (Value::String(s), Some(Indexer::Empty), value) => {
                (Self::List(vec![Value::String(s), value]), None)
            }

            (Value::String(s1), Some(Indexer::String(s2)), Value::Empty) => {
                (Self::List(vec![Value::String(s1), Value::String(s2)]), None)
            }

            // Dense list: [] or bare append
            (Value::List(mut l), Some(Indexer::Empty), value) => {
                l.push(value);
                (Value::List(l), None)
            }

            (Value::List(mut l), None, value) => {
                l.push(value);
                (Value::List(l), None)
            }

            // Dense list + explicit [n]: promote to SparseList, then insert.
            // If the result is contiguous 0..n, collapse back to a dense List.
            (Value::List(l), Some(Indexer::Number(n)), value) => {
                let mut m: BTreeMap<usize, Value<'a>> = l.into_iter().enumerate().collect();
                let err = m.entry(n).or_default().append(index_path, value).err();
                (try_densify(m), err)
            }

            (Value::List(mut l), Some(Indexer::String(s)), Value::Empty) => {
                l.push(Value::String(s));
                (Value::List(l), None)
            }

            (Value::List(l), Some(Indexer::String(s)), value) => {
                let mut error = None;
                let mut map = BTreeMap::new();

                for v in l {
                    match v {
                        Value::String(s) => {
                            map.insert(s, Value::Empty);
                        }
                        other if error.is_none() => {
                            error = Some(Error::CouldNotConvertToMap(other));
                        }
                        _ => { /*subsequent errors currently ignored*/ }
                    }
                }

                map.insert(s, value);
                (Value::Map(map), error)
            }

            // SparseList: [] or bare append (insert at last_key + 1)
            (Value::SparseList(mut m), Some(Indexer::Empty), value)
            | (Value::SparseList(mut m), None, value) => {
                let next = m.keys().last().map(|k| k + 1).unwrap_or(0);
                m.insert(next, value);
                (Value::SparseList(m), None)
            }

            // SparseList: direct insert/update at [n].
            // If the result is contiguous 0..n, collapse to a dense List.
            (Value::SparseList(mut m), Some(Indexer::Number(n)), value) => {
                let err = m.entry(n).or_default().append(index_path, value).err();
                (try_densify(m), err)
            }

            (Value::SparseList(mut m), Some(Indexer::String(s)), Value::Empty) => {
                let next = m.keys().last().map(|k| k + 1).unwrap_or(0);
                m.insert(next, Value::String(s));
                (Value::SparseList(m), None)
            }

            (Value::SparseList(m), Some(Indexer::String(s)), value) => {
                let mut error = None;
                let mut map = BTreeMap::new();

                for (_, v) in m {
                    match v {
                        Value::String(s) => {
                            map.insert(s, Value::Empty);
                        }
                        other if error.is_none() => {
                            error = Some(Error::CouldNotConvertToMap(other));
                        }
                        _ => {}
                    }
                }

                map.insert(s, value);
                (Value::Map(map), error)
            }

            (current_value, _, Value::Empty) => (current_value, None),

            // Empty + [n]: start a SparseList directly (DoS-safe)
            (Value::Empty, current_index @ Some(Indexer::Number(_)), value) => {
                Value::SparseList(BTreeMap::new()).inner_append(current_index, index_path, value)
            }

            (previous_value, indexer, new_value) => (
                previous_value.clone(),
                Some(Error::CouldNotAppend(previous_value, indexer, new_value)),
            ),
        }
    }
}

/// If the BTreeMap's keys are exactly `0..n` (contiguous from zero), convert
/// it to a dense `List`; otherwise wrap it in a `SparseList`.
///
/// The check is O(log n): BTreeMap keys are sorted, so comparing the last key
/// to `len - 1` is sufficient to determine contiguity.
fn try_densify(m: BTreeMap<usize, Value<'_>>) -> Value<'_> {
    match m.last_key_value() {
        Some((&last, _)) if last == m.len() - 1 => Value::List(m.into_values().collect()),
        _ => Value::SparseList(m),
    }
}

impl From<String> for Value<'static> {
    fn from(s: String) -> Self {
        Value::String(crate::decode(s))
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(s: &'a str) -> Self {
        Value::String(crate::decode(s))
    }
}

impl<'a> From<&'a String> for Value<'a> {
    fn from(s: &'a String) -> Self {
        Value::String(crate::decode(s))
    }
}

impl<'a, V> From<Option<V>> for Value<'a>
where
    V: Into<Value<'a>>,
{
    fn from(o: Option<V>) -> Self {
        match o {
            Some(o) => o.into(),
            None => Value::Empty,
        }
    }
}

impl<K, V> From<(K, V)> for Value<'static>
where
    K: TryInto<IndexPath<'static>>,
    K::Error: Into<Error<'static>>,
    V: Into<Value<'static>>,
{
    fn from(v: (K, V)) -> Self {
        let (key, value) = v;
        let mut v = Value::Empty;
        v.append(key, value).unwrap();
        v
    }
}

impl<'a, I: Into<Value<'a>>> From<Vec<I>> for Value<'a> {
    fn from(v: Vec<I>) -> Self {
        Value::List(v.into_iter().map(|v| v.into()).collect())
    }
}

impl From<()> for Value<'static> {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

impl PartialEq<()> for Value<'_> {
    fn eq(&self, _: &()) -> bool {
        self == &Self::Empty
    }
}

impl PartialEq<String> for Value<'_> {
    fn eq(&self, other: &String) -> bool {
        match self {
            Value::String(s) => s == other,
            _ => false,
        }
    }
}

impl PartialEq<str> for Value<'_> {
    fn eq(&self, other: &str) -> bool {
        match self {
            Value::String(s) => s == other,
            _ => false,
        }
    }
}

impl PartialEq<&str> for Value<'_> {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Value::String(s) => s == other,
            _ => false,
        }
    }
}

impl<'a: 'b, 'b> IntoIterator for &'a Value<'b> {
    type Item = (Option<IndexPath<'b>>, Option<String>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Value::Map(m) => Box::new(m.iter().flat_map(|(k1, v)| {
                v.into_iter().map(move |(k2, v)| match k2 {
                    Some(mut k2) => {
                        k2.push_front(Indexer::from(&**k1));
                        (Some(k2), v)
                    }
                    None => (Some(Indexer::from(&**k1).into()), v),
                })
            })),

            // Dense list: serializes with [] notation
            Value::List(l) => Box::new(l.iter().flat_map(|v| {
                v.into_iter().map(move |(k, v)| match k {
                    Some(mut k) => {
                        k.push_front(().into());
                        (Some(k), v)
                    }
                    None => (Some(().into()), v),
                })
            })),

            // Sparse list: serializes with [n] notation, preserving indices
            Value::SparseList(m) => Box::new(m.iter().flat_map(|(n, v)| {
                let n = *n;
                v.into_iter().map(move |(k, v)| match k {
                    Some(mut k) => {
                        k.push_front(Indexer::Number(n));
                        (Some(k), v)
                    }
                    None => (Some(Indexer::Number(n).into()), v),
                })
            })),

            Value::String(s) => Box::new(iter::once((None, Some(crate::encode(s).into_owned())))),

            Value::Empty => Box::new(iter::once((None, None))),
        }
    }
}

impl<'a, Key: TryInto<IndexPath<'a>>> Index<Key> for Value<'a> {
    type Output = Self;

    fn index(&self, key: Key) -> &Self::Output {
        self.get(key).unwrap()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Value<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Value::Map(m) => m.serialize(serializer),
            Value::List(l) => l.serialize(serializer),
            // Serializes as an object with numeric string keys e.g. {"0": "x", "2": "y"}
            Value::SparseList(m) => m.serialize(serializer),
            Value::String(s) => s.serialize(serializer),
            Value::Empty => serializer.serialize_unit(),
        }
    }
}

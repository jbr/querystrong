use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result},
};

/// A single segment of an [`IndexPath`](crate::IndexPath).
///
/// The lifetime `'a` reflects zero-copy borrowing: string keys that need no
/// percent-decoding are stored as `Cow::Borrowed` slices of the original input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Indexer<'a> {
    /// A numeric index (`[42]`), used for both dense [`List`](crate::Value::List)
    /// and sparse [`SparseList`](crate::Value::SparseList) access.
    Number(usize),
    /// A string key (`[name]` or a bare first segment like `user`).
    ///
    /// Percent-encoding and `+`-as-space substitution are applied during
    /// construction; if no decoding was necessary the original borrow is preserved.
    String(Cow<'a, str>),
    /// An empty bracket (`[]`), signalling "append to the end of the list".
    Empty,
}

impl<'a> Indexer<'a> {
    pub fn into_owned(self) -> Indexer<'static> {
        match self {
            Indexer::Number(n) => Indexer::Number(n),
            Indexer::String(cow) => Indexer::String(Cow::Owned(cow.into_owned())),
            Indexer::Empty => Indexer::Empty,
        }
    }
}

impl From<usize> for Indexer<'_> {
    fn from(f: usize) -> Self {
        Self::Number(f)
    }
}

impl From<String> for Indexer<'static> {
    fn from(f: String) -> Self {
        Self::String(crate::decode(f))
    }
}

impl<'a> From<&'a String> for Indexer<'a> {
    fn from(f: &'a String) -> Self {
        Self::String(crate::decode(f))
    }
}

impl<'a> From<Cow<'a, str>> for Indexer<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self::String(crate::decode(value))
    }
}

impl<'a> From<&'a str> for Indexer<'a> {
    fn from(f: &'a str) -> Self {
        Self::String(crate::decode(f))
    }
}

impl From<()> for Indexer<'_> {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

impl Display for Indexer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Indexer::Number(n) => f.write_str(&n.to_string()),
            Indexer::String(s) => f.write_str(&crate::encode(s)),
            Indexer::Empty => Ok(()),
        }
    }
}

use memchr::memchr2;

use crate::{Error, Indexer, Result};
use std::{
    collections::VecDeque,
    convert::TryFrom,
    fmt::{self, Display, Formatter},
    ops::{Deref, DerefMut},
    str::FromStr,
};

/// A parsed key path such as `user[name][first]`, stored as a
/// `VecDeque<`[`Indexer`]`>`.
///
/// The lifetime `'a` reflects zero-copy borrowing: string segments that need
/// no percent-decoding are stored as `Cow::Borrowed` slices of the original
/// input.
///
/// `IndexPath` implements `Deref<Target = VecDeque<Indexer<'a>>>` for direct
/// access to the underlying deque, and `Display` to render it back to
/// bracket-notation form (e.g. `user[name][first]`).
///
/// # Parsing
///
/// ```
/// use querystrong::IndexPath;
/// let path = IndexPath::parse("a[b][0]").unwrap();
/// assert_eq!(path.len(), 3); // ["a", "b", 0]
/// ```
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct IndexPath<'a>(VecDeque<Indexer<'a>>);

impl<'a> Deref for IndexPath<'a> {
    type Target = VecDeque<Indexer<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for IndexPath<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum IndexPathState {
    Start,
    BracketOpen,
    BracketClose,
    End,
}

impl FromStr for IndexPath<'static> {
    type Err = Error<'static>;

    fn from_str(s: &str) -> Result<'static, Self> {
        IndexPath::parse(s)
            .map(IndexPath::into_owned)
            .map_err(Error::into_owned)
    }
}

impl<'a> IndexPath<'a> {
    pub fn into_owned(self) -> IndexPath<'static> {
        IndexPath(self.0.into_iter().map(Indexer::into_owned).collect())
    }

    pub fn parse(mut s: &'a str) -> Result<'a, Self> {
        let orig = s;
        use IndexPathState::*;
        let mut v = VecDeque::new();
        let mut state = Start;
        loop {
            if s.is_empty() || state == End {
                break;
            }

            let (divider, current) = if let Some(divider) = memchr2(b']', b'[', s.as_bytes()) {
                // SAFETY: memchr2 guarantees `divider` is a valid byte index into `s`
                // and the matched byte is ASCII (`[` = 0x5B or `]` = 0x5D), so
                // casting it directly to char is always correct and O(1). Using
                // `chars().nth(divider)` would be O(n) and wrong for non-ASCII keys.
                let ret = (Some(s.as_bytes()[divider] as char), &s[..divider]);
                s = &s[divider + 1..];
                ret
            } else {
                (None, s)
            };

            let mut push = || {
                v.push_back(if current.is_empty() {
                    Indexer::Empty
                } else if let Ok(u) = current.parse::<usize>() {
                    Indexer::Number(u)
                } else {
                    Indexer::from(current)
                });
            };

            state = match (state, divider) {
                (_, None) => {
                    push();
                    End
                }

                (Start, Some('[')) => {
                    push();
                    BracketOpen
                }

                (BracketOpen, Some(']')) => {
                    push();
                    BracketClose
                }

                (BracketClose, Some('[')) => BracketOpen,

                _ => return Err(Error::CouldNotParseIndexer(divider, state, orig.into())),
            };
        }

        Ok(IndexPath(v))
    }
}

impl From<usize> for IndexPath<'static> {
    fn from(path: usize) -> Self {
        Self(vec![Indexer::Number(path)].into())
    }
}

impl TryFrom<String> for IndexPath<'static> {
    type Error = Error<'static>;
    fn try_from(path: String) -> Result<'static, Self> {
        IndexPath::parse(&path)
            .map_err(Error::into_owned)
            .map(IndexPath::into_owned)
    }
}

impl<'a> TryFrom<&'a str> for IndexPath<'a> {
    type Error = Error<'a>;

    fn try_from(path: &'a str) -> Result<'a, Self> {
        IndexPath::parse(path)
    }
}
impl<'a> TryFrom<&'a String> for IndexPath<'a> {
    type Error = Error<'a>;
    fn try_from(path: &'a String) -> Result<'a, Self> {
        IndexPath::parse(path.as_str())
    }
}

impl<'a, T> From<Vec<T>> for IndexPath<'a>
where
    Indexer<'a>: From<T>,
{
    fn from(other: Vec<T>) -> Self {
        Self(other.into_iter().map(Indexer::from).collect())
    }
}

impl<'a, T> From<&'a Vec<T>> for IndexPath<'a>
where
    T: 'a,
    Indexer<'a>: From<&'a T>,
{
    fn from(other: &'a Vec<T>) -> Self {
        Self(other.iter().map(Indexer::from).collect())
    }
}

impl<'a, T> PartialEq<Vec<T>> for IndexPath<'a>
where
    T: Clone,
    Indexer<'a>: From<T>,
{
    fn eq(&self, other: &Vec<T>) -> bool {
        self.0 == other.iter().map(|i| i.clone().into()).collect::<Vec<_>>()
    }
}

impl Display for IndexPath<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter();

        if let Some(first) = iter.next() {
            f.write_fmt(format_args!("{first}"))?;
        }

        for indexer in iter {
            f.write_fmt(format_args!("[{indexer}]"))?;
        }

        Ok(())
    }
}

impl From<()> for IndexPath<'static> {
    fn from(_: ()) -> Self {
        Self::from(vec![()])
    }
}

impl<'a> From<Indexer<'a>> for IndexPath<'a> {
    fn from(indexer: Indexer<'a>) -> Self {
        Self::from(vec![indexer])
    }
}

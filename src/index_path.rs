use crate::{Error, Indexer, Result};
use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
    ops::{Deref, DerefMut},
    str::FromStr,
};

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct IndexPath(VecDeque<Indexer>);

impl Deref for IndexPath {
    type Target = VecDeque<Indexer>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for IndexPath {
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

impl FromStr for IndexPath {
    type Err = Error;

    fn from_str(mut s: &str) -> Result<Self> {
        let orig = s;
        use IndexPathState::*;
        let mut v = VecDeque::new();
        let mut state = Start;
        loop {
            if s.is_empty() || state == End {
                break;
            }

            let (divider, current) = if let Some(divider) = s.find(|c| c == ']' || c == '[') {
                let ret = (Some(s.chars().nth(divider).unwrap()), &s[..divider]);
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

                _ => {
                    return Err(Error::CouldNotParseIndexer(
                        divider,
                        state,
                        orig.to_string(),
                    ))
                }
            };
        }

        Ok(IndexPath(v))
    }
}

impl From<usize> for IndexPath {
    fn from(path: usize) -> Self {
        Self(vec![Indexer::Number(path)].into())
    }
}

impl From<String> for IndexPath {
    fn from(path: String) -> Self {
        path.parse().unwrap()
    }
}

impl From<&str> for IndexPath {
    fn from(path: &str) -> Self {
        path.parse().unwrap()
    }
}
impl From<&String> for IndexPath {
    fn from(path: &String) -> Self {
        path.parse().unwrap()
    }
}

impl<T> From<Vec<T>> for IndexPath
where
    Indexer: From<T>,
{
    fn from(other: Vec<T>) -> Self {
        Self(other.into_iter().map(Indexer::from).collect())
    }
}

impl<'a, T> From<&'a Vec<T>> for IndexPath
where
    T: 'a,
    Indexer: From<&'a T>,
{
    fn from(other: &'a Vec<T>) -> Self {
        Self(other.iter().map(Indexer::from).collect())
    }
}

impl<T> PartialEq<Vec<T>> for IndexPath
where
    T: Clone,
    Indexer: From<T>,
{
    fn eq(&self, other: &Vec<T>) -> bool {
        self.0 == other.iter().map(|i| i.clone().into()).collect::<Vec<_>>()
    }
}

impl Display for IndexPath {
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

impl From<()> for IndexPath {
    fn from(_: ()) -> Self {
        Self::from(vec![()])
    }
}

impl From<Indexer> for IndexPath {
    fn from(indexer: Indexer) -> Self {
        Self::from(vec![indexer])
    }
}

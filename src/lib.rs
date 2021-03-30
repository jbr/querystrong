#![forbid(unsafe_code, future_incompatible)]
#![deny(
    missing_debug_implementations,
    nonstandard_style,
    missing_copy_implementations,
    unused_qualifications
)]

use std::fmt::{self, Debug, Display, Formatter, Write};
use std::ops::{Deref, DerefMut, Index};
use std::str::FromStr;

mod indexer;
pub use indexer::Indexer;

mod index_path;
pub use index_path::IndexPath;

mod value;
pub use value::Value;

mod error;
pub use error::Error;

#[derive(Clone, Default)]
pub struct QueryStrong(Value);

impl QueryStrong {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(s: &str) -> Result<Self, Error> {
        s.parse()
    }
}

impl FromStr for QueryStrong {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut q = QueryStrong::new();
        let parts = s.split('&').filter(|kv| !kv.is_empty()).map(|kv| {
            if let Some(eq) = kv.find('=') {
                let (k, v) = kv.split_at(eq);
                (k, Some(&v[1..]))
            } else {
                (kv, None)
            }
        });

        for (key, value) in parts {
            q.append(key, value)?;
        }

        Ok(q)
    }
}

impl Deref for QueryStrong {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for QueryStrong {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a QueryStrong {
    type Item = (IndexPath, Option<&'a str>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.into_iter().filter_map(|(k, v)| match (k, v) {
            (Some(k), Some(v)) => Some((k, Some(v))),
            (Some(k), None) => Some((k, None)),
            (None, Some(k)) => Some((
                IndexPath::from(vec![Indexer::String(String::from(k))]),
                None,
            )),
            (None, None) => None,
        }))
    }
}

impl<K> Index<K> for QueryStrong
where
    K: Into<IndexPath>,
{
    type Output = Value;

    fn index(&self, k: K) -> &Self::Output {
        self.get(k).unwrap()
    }
}

impl Display for QueryStrong {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;

        for (key, value) in self {
            if first {
                first = false;
            } else {
                f.write_char('&')?;
            }

            f.write_str(&key.to_string())?;

            if let Some(value) = value {
                f.write_char('=')?;
                f.write_str(value)?;
            }
        }
        Ok(())
    }
}

impl Debug for QueryStrong {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<F> From<F> for QueryStrong
where
    Value: From<F>,
{
    fn from(f: F) -> Self {
        Self(f.into())
    }
}

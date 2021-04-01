#![forbid(unsafe_code, future_incompatible)]
#![deny(
    missing_debug_implementations,
    nonstandard_style,
    missing_copy_implementations,
    unused_qualifications
)]

//! # QueryStrong: A flexible interface for querystrings
//!
//! Example:
//!
//! ```rust
//! # fn main() -> querystrong::Result<()> {
//! use querystrong::{QueryStrong, Value};
//! let mut qs = QueryStrong::parse("user[name][first]=jacob&user[language]=rust")?;
//! assert_eq!(qs["user[name][first]"], "jacob");
//! assert_eq!(qs["user"].get_str("language"), Some("rust"));
//! assert_eq!(qs.get_str("user[language]"), Some("rust"));
//! assert!(qs["user"].is_map());
//! assert!(qs["user[name]"].is_map());
//!
//! qs.append("user[name][last]", "rothstein")?;
//! qs.append("user[language]", "english")?;
//! assert_eq!(
//!   qs.to_string(),
//!   "user[language][]=rust&user[language][]=english&\
//!   user[name][first]=jacob&user[name][last]=rothstein"
//! );
//! # Ok(())  }
//! ```

use std::{
    fmt::{self, Debug, Display, Formatter, Write},
    ops::{Deref, DerefMut, Index},
    str::FromStr,
};

mod indexer;
pub use indexer::Indexer;

mod index_path;
pub use index_path::IndexPath;

mod value;
pub use value::Value;

mod error;
pub use error::{Error, Result};

#[derive(Clone, PartialEq, Eq)]
pub struct QueryStrong(Value);

impl QueryStrong {
    /// Creates a new (empty) querystrong that contains a map as the
    /// top level value
    pub fn new() -> Self {
        Self(Value::new_map())
    }

    /// Attempts to create a build a querystrong from the supplied querystring
    pub fn parse(s: &str) -> Result<Self> {
        s.parse()
    }
}

impl Default for QueryStrong {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for QueryStrong {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
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
    type Item = (IndexPath, Option<String>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.into_iter().filter_map(|(k, v)| match (k, v) {
            (Some(k), Some(v)) => {
                if k.is_empty() || k.get(0) == Some(&Indexer::Empty) {
                    Some((IndexPath::from(v), None))
                } else {
                    Some((k, Some(v)))
                }
            }
            (Some(k), None) => Some((k, None)),
            (None, Some(k)) => Some((Indexer::from(k).into(), None)),
            (None, None) => None,
        }))
    }
}

impl<K> Index<K> for QueryStrong
where
    K: Into<IndexPath>,
{
    type Output = Value;

    fn index(&self, key: K) -> &Self::Output {
        self.get(key).unwrap()
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
                f.write_str(&value)?;
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

impl<V: Into<Value>> From<V> for QueryStrong {
    fn from(value: V) -> Self {
        Self(value.into())
    }
}

pub(crate) fn decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into()
}

pub(crate) fn encode(s: &str) -> String {
    static ASCII_SET: percent_encoding::AsciiSet =
        percent_encoding::NON_ALPHANUMERIC.remove(b'_').remove(b'-');

    percent_encoding::utf8_percent_encode(s, &ASCII_SET).to_string()
}

#[cfg(feature = "serde")]
impl serde::Serialize for QueryStrong {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

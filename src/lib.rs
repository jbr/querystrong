#![forbid(unsafe_code, future_incompatible)]
#![deny(
    missing_debug_implementations,
    nonstandard_style,
    missing_copy_implementations,
    unused_qualifications
)]

//! # QueryStrong: A flexible interface for querystrings
//!
//! QueryStrong parses query strings (e.g. `user[name][first]=jacob&user[age]=100`)
//! into a nested [`Value`] tree that can be traversed, mutated, and serialized back
//! to a string.
//!
//! ```rust
//! use querystrong::QueryStrong;
//!
//! let mut qs = QueryStrong::parse("user[name][first]=jacob&user[language]=rust");
//! assert_eq!(qs["user[name][first]"], "jacob");
//! assert_eq!(qs.get_str("user[language]"), Some("rust"));
//! assert!(qs["user"].is_map());
//! assert!(qs["user[name]"].is_map());
//!
//! qs.append("user[name][last]", "rothstein").unwrap();
//! qs.append("user[language]", "english").unwrap();
//! assert_eq!(
//!   qs.to_string(),
//!   "user[language][]=rust&user[language][]=english&\
//!   user[name][first]=jacob&user[name][last]=rothstein"
//! );
//! ```
//!
//! ## Permissive parsing
//!
//! [`QueryStrong::parse`] never fails.  If a key cannot be parsed or a value
//! conflicts with an existing entry the error is recorded internally and
//! parsing continues.  Accumulated errors are available via
//! [`QueryStrong::errors`].
//!
//! Use [`QueryStrong::parse_strict`] if you need a hard failure on any error,
//! or call [`QueryStrong::into_result`] / [`QueryStrong::unwrap`] after the fact.
//!
//! ## Zero-copy parsing
//!
//! Parsing borrows directly from the input `&str` wherever possible.  String
//! regions that do not require percent-decoding or `+`-as-space substitution
//! are never copied.  The lifetime `'a` on [`QueryStrong<'a>`] and [`Value<'a>`]
//! tracks this borrow.  Call [`QueryStrong::into_owned`] to obtain a `'static`
//! value that owns all its strings.
//!
//! ## List variants
//!
//! Empty-bracket appends (`a[]=v`) produce a dense [`Value::List`].  Explicit
//! numeric indices (`a[3]=v`) produce a [`Value::SparseList`] backed by a
//! `BTreeMap`, which is memory-safe for large indices like `a[999999]=v`.  A
//! sparse list collapses back to a dense list automatically once its indices
//! become contiguous from zero.

use std::{
    convert::{Infallible, TryFrom, TryInto},
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
pub use error::{Error, ParseErrors, ParseResult, Result};

mod percent_coding;
pub(crate) use percent_coding::{decode, encode};

/// A parsed query string.
///
/// The lifetime `'a` is tied to the input slice supplied to [`QueryStrong::parse`].
/// String data that does not require percent-decoding is borrowed directly from
/// that slice without any allocation.  Call [`into_owned`](QueryStrong::into_owned)
/// to detach from the original string.
///
/// The top-level value is always a [`Value::Map`].  Because `QueryStrong` implements
/// [`Deref<Target = Value>`](std::ops::Deref), all [`Value`] methods are available
/// directly on a `QueryStrong`.
#[derive(Clone, PartialEq, Eq)]
pub struct QueryStrong<'a> {
    value: Value<'a>,
    errors: Option<ParseErrors<'a>>,
}

impl<'a> QueryStrong<'a> {
    /// Creates a new (empty) querystrong that contains a map as the
    /// top level value
    pub fn new() -> Self {
        Self {
            value: Value::new_map(),
            errors: None,
        }
    }

    /// Parse a query string permissively, accumulating errors rather than failing.
    ///
    /// This is the recommended entry-point for user-supplied query strings.
    /// The returned `QueryStrong` always contains the successfully-parsed portions
    /// of the input; segments that could not be parsed are skipped and their errors
    /// are stored internally.  Retrieve them with [`errors`](QueryStrong::errors).
    ///
    /// String data that does not need percent-decoding is borrowed from `s`
    /// without copying.
    ///
    /// ```
    /// use querystrong::QueryStrong;
    ///
    /// // Well-formed input: no errors
    /// let qs = QueryStrong::parse("a=1&b[c]=2");
    /// assert_eq!(qs.get_str("a"), Some("1"));
    /// assert!(qs.errors().is_none());
    ///
    /// // Conflicting segments are skipped; valid ones are preserved
    /// let qs = QueryStrong::parse("a=1&a[b]=2");
    /// assert_eq!(qs.get_str("a"), Some("1"));
    /// assert_eq!(qs.errors().unwrap().errors().len(), 1);
    /// ```
    pub fn parse(s: &'a str) -> Self {
        let mut querystrong = QueryStrong::new();
        let mut remaining = s;

        while !remaining.is_empty() {
            let kv;

            if let Some(ampersand_index) = memchr::memchr(b'&', remaining.as_bytes()) {
                kv = &remaining[..ampersand_index];
                remaining = &remaining[ampersand_index + 1..];
            } else {
                kv = remaining;
                remaining = "";
            }

            if !kv.is_empty() {
                let (k, v) = if let Some(equals_index) = memchr::memchr(b'=', kv.as_bytes()) {
                    (&kv[..equals_index], Some(&kv[equals_index + 1..]))
                } else {
                    (kv, None)
                };

                if let Err(e) = IndexPath::parse(k).and_then(|k| querystrong.append(k, v)) {
                    querystrong
                        .errors
                        .get_or_insert_with(|| ParseErrors::new(s))
                        .push(e);
                }
            }
        }

        querystrong
    }

    /// Parse a query string, returning `Err` if any part of the input is invalid.
    ///
    /// Equivalent to `QueryStrong::parse(s).into_result()`.  Prefer
    /// [`parse`](QueryStrong::parse) for untrusted inputs where a best-effort
    /// result is acceptable.
    pub fn parse_strict(s: &'a str) -> ParseResult<'a, Self> {
        Self::parse(s).into_result()
    }

    /// Returns accumulated parse errors, or `None` if parsing was clean.
    ///
    /// `None` means every key-value pair was parsed and inserted successfully.
    /// `Some(_)` means at least one segment was skipped; the successfully-parsed
    /// portions of the input are still accessible on `self`.
    pub fn errors(&self) -> Option<&ParseErrors<'a>> {
        self.errors.as_ref()
    }

    /// Convert this `QueryStrong<'a>` into a `QueryStrong<'static>` by cloning
    /// any strings that were borrowed from the original input.
    ///
    /// Useful when you need to store the parsed result beyond the lifetime of
    /// the input string.
    pub fn into_owned(self) -> QueryStrong<'static> {
        QueryStrong {
            value: self.value.into_owned(),
            errors: self.errors.map(ParseErrors::into_owned),
        }
    }

    /// Consume `self`, returning `Ok(self)` if there were no parse errors or
    /// `Err(ParseErrors)` if any were accumulated.
    ///
    /// The errors are moved out of `self` before wrapping it in `Ok`, so the
    /// returned value has a clean error state.
    pub fn into_result(mut self) -> ParseResult<'a, Self> {
        match self.errors.take() {
            Some(error) => Err(error),
            None => Ok(self),
        }
    }

    /// Panic if there were any parse errors; otherwise return `self`.
    ///
    /// Intended for tests or contexts where the input is known to be valid.
    /// For production code prefer checking [`errors`](QueryStrong::errors)
    /// or using [`parse_strict`](QueryStrong::parse_strict).
    pub fn unwrap(self) -> Self {
        self.into_result().unwrap()
    }
}

impl FromStr for QueryStrong<'static> {
    type Err = Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(QueryStrong::parse(s).into_owned())
    }
}

impl<'a> Default for QueryStrong<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Deref for QueryStrong<'a> {
    type Target = Value<'a>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a> DerefMut for QueryStrong<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<'a: 'b, 'b> IntoIterator for &'a QueryStrong<'b> {
    type Item = (IndexPath<'a>, Option<String>);

    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.value.into_iter().filter_map(|(k, v)| match (k, v) {
            (Some(k), Some(v)) => {
                if k.is_empty() || k.front() == Some(&Indexer::Empty) {
                    Some((IndexPath::try_from(v).ok()?, None))
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

impl<'a, 'b, K> Index<K> for QueryStrong<'b>
where
    K: TryInto<IndexPath<'a>>,
{
    type Output = Value<'b>;

    fn index(&self, key: K) -> &Self::Output {
        self.get(key).unwrap()
    }
}

impl Display for QueryStrong<'_> {
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

impl Debug for QueryStrong<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.value, f)
    }
}

impl<'a, V: Into<Value<'a>>> From<V> for QueryStrong<'a> {
    fn from(value: V) -> Self {
        Self {
            value: value.into(),
            errors: None,
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for QueryStrong<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

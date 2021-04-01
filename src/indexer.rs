use std::fmt::{self, Display, Formatter};

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Indexer {
    Number(usize),
    String(String),
    Empty,
}

impl From<usize> for Indexer {
    fn from(f: usize) -> Self {
        Self::Number(f)
    }
}

impl From<String> for Indexer {
    fn from(f: String) -> Self {
        Indexer::from(&f)
    }
}

impl From<&String> for Indexer {
    fn from(f: &String) -> Self {
        Indexer::from(&**f)
    }
}

impl From<&str> for Indexer {
    fn from(f: &str) -> Self {
        Self::String(percent_decode_str(f).decode_utf8_lossy().into())
    }
}

impl From<()> for Indexer {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

impl Display for Indexer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Indexer::Number(n) => f.write_str(&n.to_string()),
            Indexer::String(s) => {
                f.write_str(&utf8_percent_encode(s, NON_ALPHANUMERIC).to_string())
            }
            Indexer::Empty => Ok(()),
        }
    }
}

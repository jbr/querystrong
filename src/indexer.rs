use std::fmt::{self, Display, Formatter};

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
        Self::String(f)
    }
}

impl From<&String> for Indexer {
    fn from(f: &String) -> Self {
        Self::String(f.clone())
    }
}

impl From<&str> for Indexer {
    fn from(f: &str) -> Self {
        Self::String(String::from(f))
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
            Indexer::String(s) => f.write_str(s),
            Indexer::Empty => Ok(()),
        }
    }
}

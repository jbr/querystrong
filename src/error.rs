#[derive(Debug)]
pub struct Error(String);
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self(s)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

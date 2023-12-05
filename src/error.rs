pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Error {}
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self(s)
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

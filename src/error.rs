use std::convert::From;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use SectionKind;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    TooManySections(SectionKind),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::TooManySections(kind) => write!(f, "Too many sections of type {:?}", kind),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(err) => err.description(),
            Error::TooManySections(_) => "Too many sections",
        }
    }
}

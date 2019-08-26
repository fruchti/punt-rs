use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;

#[derive(Debug)]
pub enum Error {
    TargetNotFound,
    UnsupportedTarget,
    TooManyMatches,
    EraseError(u8),
    VerificationError,
    IoError(rusb::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::TargetNotFound => "Target not found",
            Error::UnsupportedTarget => "Target is unsupported",
            Error::TooManyMatches => "Too many matches",
            Error::EraseError(_) => "Flash erase error",
            Error::VerificationError => "Verification error",
            Error::IoError(err) => err.description(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> StdResult<(), std::fmt::Error> {
        fmt.write_str(self.description())
    }
}

impl From<rusb::Error> for Error {
    fn from(error: rusb::Error) -> Self {
        Error::IoError(error)
    }
}

pub type Result<T> = StdResult<T, Error>;

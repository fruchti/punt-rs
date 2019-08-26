use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;

/// Errors which can occur during target setup and communication.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// An operation could not be performed because it was prohibited by safety checks (e.g.
    /// programming at an odd address)
    InvalidRequest,

    /// It was attempted to open a connection to a target which does not exist.
    TargetNotFound,

    /// The given USB address pertains to an unsupported USB device (probably not even a punt
    /// bootloader).
    UnsupportedTarget,

    /// The request was not specific enough and returned in multiple matches where only a single one
    /// is supported.
    TooManyMatches,

    /// An error was reported during the erase from the target. The contained `u8` is the raw result
    /// code.
    EraseError(u8),

    /// Verifying memory contents via CRC failed.
    VerificationError,

    /// An error occurred during the raw USB communication.
    IoError(rusb::Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::InvalidRequest => "Invalid request.",
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

/// Shorthand for a Result with the crate's own Error type.
pub type Result<T> = StdResult<T, Error>;

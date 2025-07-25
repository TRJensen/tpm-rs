use core::error::Error;
use core::fmt;
use tpm2_rs_errors::*;

/// Represents success or [`MarshalingError`] failure, which is used for unmarshal/marshal functionality.
pub type MarshalingResult<T> = Result<T, MarshalingError>;

/// The MarshalingError defines Unmarshaling/Marshaling errors codes,
/// providing more explicit error codes for try_marshal* and try_unmarshal*.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum MarshalingError {
    ArrayLengthExceeded,
    MarshalingDeriveError,
    UnexpectedEndOfBuffer,
    UnknownSelector,
}

impl Error for MarshalingError {}

impl fmt::Display for MarshalingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArrayLengthExceeded => {
                write!(f, "length of array is larger than the type allows")
            }
            Self::MarshalingDeriveError => write!(f, "unexpected derive error"),
            Self::UnexpectedEndOfBuffer => {
                write!(f, "expected to have more buffer data but found none")
            }
            Self::UnknownSelector => {
                write!(f, "selector targeted is not known to the unmarshaling code")
            }
        }
    }
}

impl From<MarshalingError> for TpmRcError {
    fn from(orig: MarshalingError) -> Self {
        match orig {
            MarshalingError::ArrayLengthExceeded => TpmRcError::Size,
            MarshalingError::MarshalingDeriveError => TpmRcError::Failure,
            MarshalingError::UnexpectedEndOfBuffer => TpmRcError::Memory,
            MarshalingError::UnknownSelector => TpmRcError::Selector,
        }
    }
}

impl From<MarshalingError> for TssError {
    fn from(orig: MarshalingError) -> Self {
        match orig {
            MarshalingError::ArrayLengthExceeded => TpmRcError::Size.into(),
            MarshalingError::MarshalingDeriveError => TpmRcError::Failure.into(),
            MarshalingError::UnexpectedEndOfBuffer => TpmRcError::Memory.into(),
            MarshalingError::UnknownSelector => TpmRcError::Selector.into(),
        }
    }
}

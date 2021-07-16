use solana_program::program_error::{ProgramError, PrintProgramError};
use solana_program::decode_error::DecodeError;
use num_traits::FromPrimitive;
use solana_program::msg;

/// Errors that may be returned by the Token vesting program.
/// thiserror::Error covers std::error::Error trait
#[derive(Clone, Debug, Eq, thiserror::Error, num_derive::FromPrimitive, PartialEq)]
pub enum VestingError {
    #[error("Invalid Instruction")]
    InvalidInstruction
}

// ----------------------------------------------------------------------------- VestingError -> ProgramError
impl From<VestingError> for ProgramError {
    //todo figure out what that number means
    fn from(e: VestingError) -> Self {ProgramError::Custom(e as u32)}
}

// ----------------------------------------------------------------------------- ProgramError -> VestingError
/// so that we can do error.print::<VestingError>

impl PrintProgramError for VestingError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            VestingError::InvalidInstruction => msg!("Error: Invalid instruction!"),
        }
    }
}

impl<T> DecodeError<T> for VestingError {
    fn type_of() -> &'static str {
        "VestingError"
    }
}

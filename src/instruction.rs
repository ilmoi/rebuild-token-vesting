use solana_program::{program_error::ProgramError, msg};
use crate::error::VestingError::InvalidInstruction;
use std::convert::TryInto;
use crate::error::VestingError;

#[derive(Clone, Debug, PartialEq)]
pub enum VestingInstruction {
    /// Initializes an empty program account for the token_vesting program
    ///
    /// Accounts expected by this instruction:
    ///
    ///   * Single owner
    ///   0. `[]` The system program account
    ///   1. `[]` The sysvar Rent account
    ///   1. `[signer]` The fee payer account
    ///   1. `[]` The vesting account
    Init {
        // The seed used to derive the vesting accounts address
        seeds: [u8; 32],
        // The number of release schedules for this contract to hold
        number_of_schedules: u32,
    },
}

impl VestingInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(InvalidInstruction)?;

        let result = match tag {
            0 => {
                let seeds = Self::unpack_seeds(rest, 0, 32).unwrap();
                let number_of_schedules = Self::unpack_u32(rest, 32)?;
                Self::Init {
                    seeds,
                    number_of_schedules,
                }
            }
            _ => {
                msg!("unsupported instruction! passed tag: {:?}", tag);
                return Err(InvalidInstruction.into());
            }
        };

        Ok(result)
    }

    fn unpack_seeds(rest: &[u8], start: usize, end: usize) -> Option<[u8; 32]> {
        rest
            .get(start..end) //32 bytes of seeds
            .and_then(|slice| slice.try_into().ok())
    }

    fn unpack_u32(rest: &[u8], start: usize) -> Result<u32, VestingError> {
        rest
            .get(start..start+4) //4 bytes int
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes) //todo surprised they're using LE here not BE?
            .ok_or(InvalidInstruction)
    }
}
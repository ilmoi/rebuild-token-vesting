use solana_program::{program_error::ProgramError, msg};
use crate::error::VestingError::InvalidInstruction;
use std::convert::TryInto;
use crate::error::VestingError;
use solana_program::pubkey::Pubkey;

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
    /// Creates a new vesting schedule contract
    ///
    /// Accounts expected by this instruction:
    ///
    ///   * Single owner
    ///   0. `[]` The spl-token program account
    ///   1. `[writable]` The vesting account
    ///   2. `[writable]` The vesting spl-token account
    ///   3. `[signer]` The source spl-token account owner
    ///   4. `[writable]` The source spl-token account
    Create {
        seeds: [u8; 32],
        token_mint_addr: Pubkey,
        token_dest_addr: Pubkey,
        schedules: Vec<Schedule>,
    },
    /// Unlocks a simple vesting contract (SVC) - can only be invoked by the program itself
    /// Accounts expected by this instruction:
    ///
    ///   * Single owner
    ///   0. `[]` The spl-token program account
    ///   1. `[]` The clock sysvar account
    ///   1. `[writable]` The vesting account
    ///   2. `[writable]` The vesting spl-token account
    ///   3. `[writable]` The destination spl-token account
    Unlock { seeds: [u8; 32] },

    /// Change the destination account of a given simple vesting contract (SVC)
    /// - can only be invoked by the present destination address of the contract.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   * Single owner
    ///   0. `[]` The vesting account
    ///   1. `[]` The current destination token account
    ///   2. `[signer]` The destination spl-token account owner
    ///   3. `[]` The new destination spl-token account
    ChangeDestination { seeds: [u8; 32] },
}

pub const SCHEDULE_SIZE: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    pub release_time: u64, //in SECONDS, not milliseconds
    pub amount: u64,
}


impl VestingInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(InvalidInstruction)?;

        let result = match tag {
            0 => {
                let seeds = Self::unpack_seeds(rest, 0).unwrap();
                let number_of_schedules = Self::unpack_u32(rest, 32)?;
                Self::Init {
                    seeds,
                    number_of_schedules,
                }
            }
            1 => {
                let seeds = Self::unpack_seeds(rest, 0).unwrap();
                let token_mint_addr = Self::unpack_addr(rest, 32)?;
                let token_dest_addr = Self::unpack_addr(rest, 64)?;

                let number_of_schedules = rest[96..].len() / SCHEDULE_SIZE;
                let mut schedules: Vec<Schedule> = Vec::with_capacity(number_of_schedules);
                let mut offset = 96;

                for _ in 0..number_of_schedules {
                    let release_time = Self::unpack_u64(rest, offset)?;
                    let amount = Self::unpack_u64(rest, offset+8)?;
                    offset += SCHEDULE_SIZE;
                    schedules.push(Schedule {release_time, amount})
                }

                Self::Create {
                    seeds,
                    token_mint_addr,
                    token_dest_addr,
                    schedules,
                }
            }
            2 | 3 => {
                let seeds = Self::unpack_seeds(rest, 0).unwrap();
                match tag {
                    2 => Self::Unlock {seeds},
                    _ => Self::ChangeDestination {seeds},
                }
            }
            _ => {
                msg!("unsupported instruction! passed tag: {:?}", tag);
                return Err(InvalidInstruction.into());
            }
        };

        Ok(result)
    }

    /// assumes 32 bytes long
    fn unpack_seeds(rest: &[u8], start: usize) -> Option<[u8; 32]> {
        rest
            .get(start..start+32) //32 bytes of seeds
            .and_then(|slice| slice.try_into().ok())
    }

    fn unpack_u32(rest: &[u8], start: usize) -> Result<u32, VestingError> {
        rest
            .get(start..start+4) //4 bytes int
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes) //todo surprised they're using LE here not BE?
            .ok_or(InvalidInstruction)
    }

    fn unpack_u64(rest: &[u8], start: usize) -> Result<u64, VestingError> {
        rest
            .get(start..start+8) //8 bytes int
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes) //todo surprised they're using LE here not BE?
            .ok_or(InvalidInstruction)
    }

    fn unpack_addr(rest: &[u8], start:usize) -> Result<Pubkey, VestingError> {
        rest
            .get(start..start+32)
            .and_then(|slice| slice.try_into().ok())
            .map(Pubkey::new)
            .ok_or(InvalidInstruction)
    }
}
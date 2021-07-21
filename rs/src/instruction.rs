// use borsh::{BorshDeserialize, BorshSerialize};
// use serde::{Deserialize, Serialize};
use std::{convert::TryInto, mem::size_of};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    log::sol_log_compute_units,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::error::{VestingError, VestingError::InvalidInstruction};

pub type Seeds = [u8; 32];

// #[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
// #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
        seeds: Seeds,
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
        seeds: Seeds,
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
    Unlock {
        seeds: Seeds,
    },

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
    ChangeDestination {
        seeds: Seeds,
    },
    Empty {
        number: u32,
    },
}

pub const SCHEDULE_SIZE: usize = 16;

// #[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
// #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
pub struct Schedule {
    pub release_time: u64, //in SECONDS, not milliseconds
    pub amount: u64,
}

impl VestingInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        msg!("input is {:?}", input);

        // Below are listed 3 different ways of deserializing the incoming byte array.
        // Uncomment the appropriate one.
        // you might have to derive Serialize, Deserialize / BorshSerialize, BorshDeserialize on a few structs/enums to make the code compile

        // ----------------------------------------------------------------------------- 1 manual
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
                    let amount = Self::unpack_u64(rest, offset + 8)?;
                    offset += SCHEDULE_SIZE;
                    schedules.push(Schedule {
                        release_time,
                        amount,
                    })
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
                    2 => Self::Unlock { seeds },
                    _ => Self::ChangeDestination { seeds },
                }
            }
            4 => {
                let number = Self::unpack_u32(rest, 0).unwrap();
                Self::Empty { number }
            }
            _ => {
                msg!("unsupported instruction! passed tag: {:?}", tag);
                return Err(InvalidInstruction.into());
            }
        };

        // ----------------------------------------------------------------------------- 2 bincode
        // let result: Self = bincode::deserialize(input).unwrap();

        // ----------------------------------------------------------------------------- 3 borsh
        // let result: Self = Self::try_from_slice(input).unwrap();

        // -----------------------------------------------------------------------------
        msg!("result is {:?}", result);
        sol_log_compute_units();
        Ok(result)
    }

    /// assumes 32 bytes long
    fn unpack_seeds(rest: &[u8], start: usize) -> Option<Seeds> {
        rest.get(start..start + 32) //32 bytes of seeds
            .and_then(|slice| slice.try_into().ok())
    }

    fn unpack_u32(rest: &[u8], start: usize) -> Result<u32, VestingError> {
        rest.get(start..start + 4) //4 bytes int
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(InvalidInstruction)
    }

    fn unpack_u64(rest: &[u8], start: usize) -> Result<u64, VestingError> {
        // return Err(VestingError::SomeOther);
        rest.get(start..start + 8) //8 bytes int
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(InvalidInstruction)
    }

    fn unpack_addr(rest: &[u8], start: usize) -> Result<Pubkey, VestingError> {
        rest.get(start..start + 32)
            .and_then(|slice| slice.try_into().ok())
            .map(Pubkey::new)
            .ok_or(InvalidInstruction)
    }

    // the reverse of above - packs an instruction into a vector of bytes
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match self {
            Self::Empty { number } => return vec![0],
            &Self::Init {
                seeds,
                number_of_schedules,
            } => {
                buf.push(0);
                buf.extend_from_slice(&seeds);
                buf.extend_from_slice(&number_of_schedules.to_le_bytes())
            }
            Self::Create {
                seeds,
                token_mint_addr,
                token_dest_addr,
                schedules,
            } => {
                buf.push(1);
                buf.extend_from_slice(seeds);
                buf.extend_from_slice(&token_mint_addr.to_bytes());
                buf.extend_from_slice(&token_dest_addr.to_bytes());
                for s in schedules.iter() {
                    buf.extend_from_slice(&s.release_time.to_le_bytes());
                    buf.extend_from_slice(&s.amount.to_le_bytes());
                }
            }
            &Self::Unlock { seeds } => {
                buf.push(2);
                buf.extend_from_slice(&seeds);
            }
            &Self::ChangeDestination { seeds } => {
                buf.push(3);
                buf.extend_from_slice(&seeds);
            }
        };
        buf
    }
}

// ----------------------------------------------------------------------------- helper fns to be called from tests / other rust code

// Creates a `Init` instruction
pub fn init(
    system_program_id: &Pubkey,
    rent_program_id: &Pubkey,
    vesting_program_id: &Pubkey,
    payer_key: &Pubkey,
    vesting_account: &Pubkey,
    seeds: Seeds,
    number_of_schedules: u32,
) -> Result<Instruction, ProgramError> {
    let data = VestingInstruction::Init {
        seeds,
        number_of_schedules,
    }
    .pack();
    let accounts = vec![
        AccountMeta::new_readonly(*system_program_id, false),
        AccountMeta::new_readonly(*rent_program_id, false),
        AccountMeta::new(*payer_key, true),
        AccountMeta::new(*vesting_account, false),
    ];
    Ok(Instruction {
        program_id: *vesting_program_id,
        accounts,
        data,
    })
}

// Creates a `CreateSchedule` instruction
pub fn create(
    vesting_program_id: &Pubkey,
    token_program_id: &Pubkey,
    vesting_account_key: &Pubkey,
    vesting_token_account_key: &Pubkey,
    source_token_account_owner_key: &Pubkey,
    source_token_account_key: &Pubkey,
    destination_token_account_key: &Pubkey,
    mint_address: &Pubkey,
    schedules: Vec<Schedule>,
    seeds: Seeds,
) -> Result<Instruction, ProgramError> {
    let data = VestingInstruction::Create {
        token_mint_addr: *mint_address,
        seeds,
        token_dest_addr: *destination_token_account_key,
        schedules,
    }
    .pack();
    let accounts = vec![
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*vesting_account_key, false),
        AccountMeta::new(*vesting_token_account_key, false),
        AccountMeta::new_readonly(*source_token_account_owner_key, true),
        AccountMeta::new(*source_token_account_key, false),
    ];
    Ok(Instruction {
        program_id: *vesting_program_id,
        accounts,
        data,
    })
}

// Creates an `Unlock` instruction
pub fn unlock(
    vesting_program_id: &Pubkey,
    token_program_id: &Pubkey,
    clock_sysvar_id: &Pubkey,
    vesting_account_key: &Pubkey,
    vesting_token_account_key: &Pubkey,
    destination_token_account_key: &Pubkey,
    seeds: Seeds,
) -> Result<Instruction, ProgramError> {
    let data = VestingInstruction::Unlock { seeds }.pack();
    let accounts = vec![
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*clock_sysvar_id, false),
        AccountMeta::new(*vesting_account_key, false),
        AccountMeta::new(*vesting_token_account_key, false),
        AccountMeta::new(*destination_token_account_key, false),
    ];
    Ok(Instruction {
        program_id: *vesting_program_id,
        accounts,
        data,
    })
}

pub fn change_destination(
    vesting_program_id: &Pubkey,
    vesting_account_key: &Pubkey,
    current_destination_token_account_owner: &Pubkey,
    current_destination_token_account: &Pubkey,
    target_destination_token_account: &Pubkey,
    seeds: Seeds,
) -> Result<Instruction, ProgramError> {
    let data = VestingInstruction::ChangeDestination { seeds }.pack();
    let accounts = vec![
        AccountMeta::new(*vesting_account_key, false),
        AccountMeta::new_readonly(*current_destination_token_account, false),
        AccountMeta::new_readonly(*current_destination_token_account_owner, true),
        AccountMeta::new_readonly(*target_destination_token_account, false),
    ];
    Ok(Instruction {
        program_id: *vesting_program_id,
        accounts,
        data,
    })
}

// ----------------------------------------------------------------------------- needed for fuzzing

#[cfg(feature = "fuzz")]
impl arbitrary::Arbitrary<'_> for VestingInstruction {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let seeds: [u8; 32] = u.arbitrary()?;
        let choice = u.choose(&[0, 1, 2, 3, 4])?;
        match choice {
            0 => {
                let number_of_schedules = u.arbitrary()?;
                return Ok(Self::Init {
                    seeds,
                    number_of_schedules,
                });
            }
            _ => {
                let schedules: [Schedule; 10] = u.arbitrary()?;
                let key_bytes: [u8; 32] = u.arbitrary()?;
                let token_mint_addr: Pubkey = Pubkey::new(&key_bytes);
                let key_bytes: [u8; 32] = u.arbitrary()?;
                let token_dest_addr: Pubkey = Pubkey::new(&key_bytes);
                return Ok(Self::Create {
                    seeds,
                    token_mint_addr,
                    token_dest_addr,
                    schedules: schedules.to_vec(),
                });
            } // todo didn't bother implementing..
              // 2 => return Ok(Self::Unlock { seeds }),
              // 3 => return Ok(Self::ChangeDestination { seeds }),
              // _ => {
              //     return Ok(Self::Empty {
              //         number: u.arbitrary()?,
              //     })
              // }
        }
    }
}

// ----------------------------------------------------------------------------- test

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instruction_packing() {
        let token_mint_addr = Pubkey::new_unique();
        let token_dest_addr = Pubkey::new_unique();

        let original_create = VestingInstruction::Create {
            seeds: [50u8; 32],
            schedules: vec![Schedule {
                amount: 42,
                release_time: 250,
            }],
            token_mint_addr: token_mint_addr.clone(),
            token_dest_addr,
        };
        let packed_create = original_create.pack();
        let unpacked_create = VestingInstruction::unpack(&packed_create).unwrap();
        assert_eq!(original_create, unpacked_create);

        let original_unlock = VestingInstruction::Unlock { seeds: [50u8; 32] };
        assert_eq!(
            original_unlock,
            VestingInstruction::unpack(&original_unlock.pack()).unwrap()
        );

        let original_init = VestingInstruction::Init {
            number_of_schedules: 42,
            seeds: [50u8; 32],
        };
        assert_eq!(
            original_init,
            VestingInstruction::unpack(&original_init.pack()).unwrap()
        );

        let original_change = VestingInstruction::ChangeDestination { seeds: [50u8; 32] };
        assert_eq!(
            original_change,
            VestingInstruction::unpack(&original_change.pack()).unwrap()
        );
    }
}

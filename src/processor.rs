use solana_program::pubkey::Pubkey;
use solana_program::account_info::{AccountInfo, next_account_info};
use solana_program::{entrypoint::ProgramResult, msg};
use crate::instruction::VestingInstruction;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::create_account;
use solana_program::program::invoke_signed;
use crate::state::{VestingSchedule, VestingScheduleHeader};
use solana_program::program_pack::Pack;

pub struct Processor {}

impl Processor {
    pub fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        msg!("begin processing ix");

        // decode the instruction from bytes
        let instruction = VestingInstruction::unpack(instruction_data)?;

        // match the decoded instruction
        match instruction {
            VestingInstruction::Init {seeds, number_of_schedules} => {
                msg!("Instruction: Init");
                Self::process_init(program_id, accounts, seeds, number_of_schedules)
            }
        }
    }

    fn process_init(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        seeds: [u8; 32],
        number_of_schedules: u32,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let system_program_account = next_account_info(accounts_iter)?;
        let rent_sysvar_account = next_account_info(accounts_iter)?;
        let payer = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?;

        // ----------------------------------------------------------------------------- size & rent
        let state_size = (number_of_schedules as usize) * VestingSchedule::LEN + VestingScheduleHeader::LEN;
        let rent = Rent::from_account_info(rent_sysvar_account)?;
        let rent_size = rent.minimum_balance(state_size);

        // ----------------------------------------------------------------------------- vesting account key
        // find the non reversible public key for the vesting contract via the seed + check against the one that was passed
        // in other words, vesting_account = PDA of the vesting program
        let vesting_account_key = Pubkey::create_program_address(&[&seeds], &program_id).unwrap();
        if vesting_account_key != *vesting_account.key {
            msg!("Provided vesting account is invalid");
            return Err(ProgramError::InvalidArgument);
        }

        // ----------------------------------------------------------------------------- create
        // ask system_program to create the actual account with the right space and rent
        let init_vesting_account = create_account(
            &payer.key,
            &vesting_account_key,
            rent_size,
            state_size as u64,
            &program_id,
        );

        invoke_signed( //note how we're using _signed coz it's a PDA
            &init_vesting_account,
            &[
                system_program_account.clone(),
                payer.clone(),
                vesting_account.clone(),
            ],
            &[&[&seeds]], //signing with seeds
        )?;
        Ok(())
    }
}


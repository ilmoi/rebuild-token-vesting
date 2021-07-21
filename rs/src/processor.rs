use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::create_account,
    sysvar::Sysvar,
};
use spl_token::{instruction::transfer, state::Account};

use crate::{
    instruction::{Schedule, Seeds, VestingInstruction, SCHEDULE_SIZE},
    state::{pack_schedules_into_slice, unpack_schedules, VestingSchedule, VestingScheduleHeader},
};

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
            VestingInstruction::Empty { number } => {
                msg!("it worked, number is {}", number);
                return Ok(());
            }
            VestingInstruction::Init {
                seeds,
                number_of_schedules,
            } => {
                msg!("Instruction: Init");
                Self::process_init(program_id, accounts, seeds, number_of_schedules)
            }
            VestingInstruction::Create {
                seeds,
                token_mint_addr,
                token_dest_addr,
                schedules,
            } => {
                msg!("Instruction: Create");
                Self::process_create(
                    program_id,
                    accounts,
                    seeds,
                    &token_mint_addr,
                    &token_dest_addr,
                    schedules,
                )
            }
            VestingInstruction::Unlock { seeds } => {
                msg!("Instruction: Unlock");
                Self::process_unlock(program_id, accounts, seeds)
            }
            VestingInstruction::ChangeDestination { seeds } => {
                msg!("Instruction: Change Destination");
                Self::process_change_destination(program_id, accounts, seeds)
            }
        }
    }

    fn process_init(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        seeds: Seeds,
        number_of_schedules: u32,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let system_program_account = next_account_info(accounts_iter)?;
        let rent_sysvar_account = next_account_info(accounts_iter)?;
        let payer = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?;

        // ----------------------------------------------------------------------------- size & rent
        let state_size =
            (number_of_schedules as usize) * VestingSchedule::LEN + VestingScheduleHeader::LEN;
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

        invoke_signed(
            //note how we're using _signed coz it's a PDA
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

    pub fn process_create(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        seeds: Seeds,
        token_mint_addr: &Pubkey,
        token_dest_addr: &Pubkey,
        schedules: Vec<Schedule>,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let spl_token_account = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?; //the one that holds the info
        let vesting_token_account = next_account_info(accounts_iter)?; //the one that will hold the tokens
        let source_token_account_owner = next_account_info(accounts_iter)?;
        let source_token_account = next_account_info(accounts_iter)?;

        // ----------------------------------------------------------------------------- checks
        // check passed in vesting account's addr matches derived PDA addr
        let vesting_account_key = Pubkey::create_program_address(&[&seeds], program_id)?;
        if vesting_account_key != *vesting_account.key {
            msg!("bad provided vesting account");
            return Err(ProgramError::InvalidArgument);
        }

        if !source_token_account_owner.is_signer {
            msg!("source token account owner should be a signer");
            return Err(ProgramError::MissingRequiredSignature);
        }

        if *vesting_account.owner != *program_id {
            msg!("vesting account should be owned by the vesting program");
            return Err(ProgramError::InvalidArgument);
        }

        // take the last byte of the header
        let is_initialized =
            vesting_account.try_borrow_data()?[VestingScheduleHeader::LEN - 1] == 1;
        if is_initialized {
            msg!("cannot overwrite an existing vesting contract");
            return Err(ProgramError::InvalidArgument);
        }

        // because this is an instance of TokenAccount, we can unpack it with a predefined function
        let vesting_token_account_data = Account::unpack(&vesting_token_account.data.borrow())?;

        // so what we want is:
        // (vesting) program_id -> owns vesting_account
        // vesting_account -> owns vesting_token_account
        if vesting_token_account_data.owner != *vesting_account.key {
            msg!("vesting token account should be owned by vesting account");
            return Err(ProgramError::InvalidArgument);
        }

        if vesting_token_account_data.delegate.is_some() {
            msg!("vesting account should NOT have a delegate");
            return Err(ProgramError::InvalidAccountData);
        }

        if vesting_token_account_data.close_authority.is_some() {
            msg!("vesting account should NOT have a close authority");
            return Err(ProgramError::InvalidAccountData);
        }

        // ----------------------------------------------------------------------------- update state
        //the reason we're creating a new one instead of deserializing existing one is because THERE IS NO EXISTING ONE
        //one of the checks above makes sure that (the one that checks is_initialized is false)
        let state_header = VestingScheduleHeader {
            destination_address: *token_dest_addr,
            mint_address: *token_mint_addr,
            is_initialized: true,
        };

        //get a mutable reference to vesting_account's data
        let mut data = vesting_account.data.borrow_mut();
        if data.len() != VestingScheduleHeader::LEN + schedules.len() * VestingSchedule::LEN {
            msg!(
                "data len not right: l = {:?}, r = {:?}",
                data.len(),
                VestingScheduleHeader::LEN + schedules.len() * VestingSchedule::LEN
            );
            return Err(ProgramError::InvalidAccountData);
        }

        //pack the newly created header into that reference
        state_header.pack_into_slice(&mut data);

        // ----------------------------------------------------------------------------- build up amount

        let mut offset = VestingScheduleHeader::LEN; //needed to pack schedule into data
        let mut total_amount: u64 = 0; //needed to keep track of total amount

        for s in schedules.iter() {
            let state_schedule = VestingSchedule {
                release_time: s.release_time,
                amount: s.amount,
            };
            //we're packing the schedule at a specific offset
            state_schedule.pack_into_slice(&mut data[offset..]);

            let delta = total_amount.checked_add(s.amount);
            match delta {
                Some(n) => total_amount = n, //not +=n, we're doing checked_add above
                None => return Err(ProgramError::InvalidInstructionData),
            }
            offset += SCHEDULE_SIZE;
        }

        //if existing amount in source token below total amount, we can't do it
        if Account::unpack(&source_token_account.data.borrow())?.amount < total_amount {
            msg!("source token account has insufficient funds");
            return Err(ProgramError::InsufficientFunds);
        }

        // ----------------------------------------------------------------------------- send funds

        let transfer_tokens_from_source_to_vesting_ix = transfer(
            spl_token_account.key,
            source_token_account.key,
            vesting_token_account.key,
            source_token_account_owner.key,
            &[], //not a multisig account that's why this is empty
            total_amount,
        )?;

        invoke(
            //not invoke_signed because it's alice who's signing and not a PDA
            &transfer_tokens_from_source_to_vesting_ix,
            &[
                source_token_account.clone(),
                vesting_token_account.clone(),
                spl_token_account.clone(),
                source_token_account_owner.clone(),
            ],
        )?;

        Ok(())
    }

    pub fn process_unlock(
        program_id: &Pubkey,
        _accounts: &[AccountInfo],
        seeds: Seeds,
    ) -> ProgramResult {
        let accounts_iter = &mut _accounts.iter();

        let spl_token_account = next_account_info(accounts_iter)?;
        let clock_sysvar_account = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?; //this is the one with the headers and schedules
        let vesting_token_account = next_account_info(accounts_iter)?; //this is the one with the tokens
        let destination_token_account = next_account_info(accounts_iter)?;

        // ----------------------------------------------------------------------------- checks
        //check passed vesting account matches derived vesting account
        let vesting_account_key = Pubkey::create_program_address(&[&seeds], program_id)?;
        if vesting_account_key != *vesting_account.key {
            msg!("Invalid vesting account key");
            return Err(ProgramError::InvalidArgument);
        }

        //check provided spl_token program is the real one
        if spl_token_account.key != &spl_token::id() {
            msg!("The provided spl token program account is invalid");
            return Err(ProgramError::InvalidArgument);
        }

        // unpack header
        let packed_state = &vesting_account.data;
        let header_state =
            VestingScheduleHeader::unpack(&packed_state.borrow()[..VestingScheduleHeader::LEN])?;

        // check that header's dest addr matches provided dest addr
        if header_state.destination_address != *destination_token_account.key {
            msg!("Contract destination account does not matched provided account");
            return Err(ProgramError::InvalidArgument);
        }

        // unpack vesting token account
        let vesting_token_account_data = Account::unpack(&vesting_token_account.data.borrow())?;

        // check the owner of that account is the vesting_account
        if vesting_token_account_data.owner != vesting_account_key {
            msg!("The vesting token account should be owned by the vesting account.");
            return Err(ProgramError::InvalidArgument);
        }

        // ----------------------------------------------------------------------------- core
        // figure out how much has vested and can be transferred
        let clock = Clock::from_account_info(&clock_sysvar_account)?;
        let mut total_amount_to_transfer = 0;
        let mut schedules = unpack_schedules(&packed_state.borrow()[VestingScheduleHeader::LEN..])?;

        for s in schedules.iter_mut() {
            msg!(
                "unix timestamp: {:?}, schedule's release time: {:?}",
                clock.unix_timestamp as u64,
                s.release_time
            );
            if clock.unix_timestamp as u64 >= s.release_time {
                total_amount_to_transfer += s.amount;
                s.amount = 0; //note we're also setting the amount to 0. we will update state below. this is so that once an amount has vested, it only transfers out of the vesting contract ONCE
            }
        }
        if total_amount_to_transfer == 0 {
            msg!("Vesting contract has not yet reached release time");
            return Err(ProgramError::InvalidArgument);
        }

        msg!(
            "vesting contract balance is {:?}",
            vesting_token_account_data.amount
        );
        msg!("total amount to transfer is {:?}", total_amount_to_transfer);

        // ----------------------------------------------------------------------------- transfer
        let transfer_tokens_from_vesting_account = transfer(
            &spl_token_account.key,
            &vesting_token_account.key,
            destination_token_account.key,
            &vesting_account_key,
            &[],
            total_amount_to_transfer,
        )?;

        invoke_signed(
            //sign with a pda coz token_vesting_account is a pda
            &transfer_tokens_from_vesting_account,
            &[
                spl_token_account.clone(),
                vesting_token_account.clone(),
                destination_token_account.clone(),
                vesting_account.clone(),
            ],
            &[&[&seeds]],
        )?;

        // ----------------------------------------------------------------------------- update state
        // Reset released amounts to 0. This makes the simple unlock safe with complex scheduling contracts
        pack_schedules_into_slice(
            schedules,
            &mut packed_state.borrow_mut()[VestingScheduleHeader::LEN..],
        );

        Ok(())
    }

    pub fn process_change_destination(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        seeds: Seeds,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let vesting_account = next_account_info(accounts_iter)?;
        let destination_token_account = next_account_info(accounts_iter)?;
        let destination_token_account_owner = next_account_info(accounts_iter)?;
        let new_destination_token_account = next_account_info(accounts_iter)?;

        // ----------------------------------------------------------------------------- checks
        if vesting_account.data.borrow().len() < VestingScheduleHeader::LEN {
            msg!("vesting account's data should  never be shorter than the header");
            return Err(ProgramError::InvalidAccountData);
        }

        // check vesting account matches
        let vesting_account_key = Pubkey::create_program_address(&[&seeds], program_id)?;
        if vesting_account_key != *vesting_account.key {
            msg!("Invalid vesting account key");
            return Err(ProgramError::InvalidArgument);
        }

        // check destination account matches
        let state = VestingScheduleHeader::unpack(
            &vesting_account.data.borrow()[..VestingScheduleHeader::LEN],
        )?;

        if state.destination_address != *destination_token_account.key {
            msg!("Contract destination account does not matched provided account");
            return Err(ProgramError::InvalidArgument);
        }

        // check signer (dest acc) present
        if !destination_token_account_owner.is_signer {
            msg!("Destination token account owner should be a signer.");
            return Err(ProgramError::InvalidArgument);
        }

        let destination_token_account = Account::unpack(&destination_token_account.data.borrow())?;
        if destination_token_account.owner != *destination_token_account_owner.key {
            msg!("The current destination token account isn't owned by the provided owner");
            return Err(ProgramError::InvalidArgument);
        }

        // ----------------------------------------------------------------------------- core
        //get a mutable copy of state
        let mut new_state = state;
        //update the address
        new_state.destination_address = *new_destination_token_account.key;
        //pack into state of vesting account
        new_state
            .pack_into_slice(&mut vesting_account.data.borrow_mut()[..VestingScheduleHeader::LEN]);

        Ok(())
    }
}

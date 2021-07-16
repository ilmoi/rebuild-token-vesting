//todo is there no addon/crate that auto-optimizes imports?
use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    msg,
    entrypoint,
    entrypoint::ProgramResult,
};

use crate::{processor::Processor, error::VestingError};
use solana_program::program_error::PrintProgramError;

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Entrypoint!");

    if let Err(e) = Processor::process_instruction(program_id, accounts, instruction_data) {

        // todo 1)what is e.print? 2)how do I benefit by casting?
        // e.print(); //won't work without type annotation
        e.print::<VestingError>();
        return Err(e);
    }

    Ok(())
}

// todo interesting, can specify a specific addr to deploy to?
// Deploy the program with the following id:
// solana_program::declare_id!("VestingbGKPFXCWuBvfkegQfZyiNwAJb9Ss623VQ5DA");


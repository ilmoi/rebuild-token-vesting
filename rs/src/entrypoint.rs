use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::PrintProgramError, pubkey::Pubkey,
};

use crate::{error::VestingError, processor::Processor};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Entrypoint!");

    if let Err(e) = Processor::process_instruction(program_id, accounts, instruction_data) {
        // casting into VestingError means the error msg from vesting error will get printed
        // e.print(); //won't work without type annotation
        e.print::<VestingError>();
        return Err(e);
    }

    Ok(())
}

// Deploy the program with the following id:
solana_program::declare_id!("SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM");

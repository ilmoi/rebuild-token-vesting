use std::{borrow::Borrow, collections::HashMap, convert::TryInto, str::FromStr};

use honggfuzz::fuzz;
use rebuild_rs::{
    instruction::{create, init, unlock, Schedule, VestingInstruction},
    processor::Processor,
    state::VestingSchedule,
};
use solana_program::{
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    system_instruction::create_account,
    system_program,
    sysvar::{self, SysvarId},
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    hash::Hash,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
    transport::TransportError,
};
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    solana_program::program_pack::Pack,
};

// ----------------------------------------------------------------------------- structs / consts

pub struct TokenVestingEnv {
    system_program_id: Pubkey,
    token_program_id: Pubkey,
    clock_program_id: Pubkey,
    rent_program_id: Pubkey,
    vesting_program_id: Pubkey,
    mint_authority_keypair: Keypair,
}

#[derive(Debug, arbitrary::Arbitrary, Clone)]
pub struct FuzzInstruction {
    instruction: VestingInstruction, // these seeds in this ix won't be correct but it doesn't matter, we're only using it for matching, to decide with ix to perform
    amount: u64,
    seeds: [u8; 32],
    vesting_account_key: AccountId,
    vesting_token_account_key: AccountId,
    source_token_account_owner_key: AccountId,
    source_token_account_key: AccountId,
    source_token_amount: u64,
    destination_token_owner_key: AccountId,
    destination_token_key: AccountId,
    new_destination_token_key: AccountId,
    mint_key: AccountId,
    schedules: Vec<Schedule>,
    payer_key: AccountId,
    vesting_program_account: AccountId,
    number_of_schedules: u8,
    // This flag decides wether the instruction will be executed with inputs that should
    // not provoke any errors. (The accounts and contracts will be set up before if needed)
    correct_inputs: bool,
}

/// Use u8 as an account id to simplify the address space and re-use accounts
/// more often.
type AccountId = u8;

// ----------------------------------------------------------------------------- fuzz main

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let token_vesting_testenv = TokenVestingEnv {
        system_program_id: system_program::id(),
        token_program_id: spl_token::id(),
        clock_program_id: sysvar::clock::id(),
        rent_program_id: sysvar::rent::id(),
        vesting_program_id: Pubkey::from_str("SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM")
            .unwrap(),
        mint_authority_keypair: Keypair::new(),
    };

    loop {
        // the fuzzer can generate any number of instructions - 0, 1, 2, more...
        fuzz!(|fuzz_instructions: Vec<FuzzInstruction>| {
            println!("ix are: {:?}", fuzz_instructions);
            // if fuzz_instructions.len() >= 2 {
            //     panic!();
            // }

            let mut program_test = ProgramTest::new(
                "rebuild_rs",
                token_vesting_testenv.vesting_program_id,
                processor!(Processor::process_instruction),
            );

            program_test.add_account(
                token_vesting_testenv.mint_authority_keypair.pubkey(),
                Account {
                    lamports: u32::MAX as u64,
                    ..Account::default()
                },
            );

            let mut test_state = rt.block_on(program_test.start_with_context());

            rt.block_on(run_fuzz_instructions(
                &token_vesting_testenv,
                &mut test_state.banks_client,
                test_state.payer,
                test_state.last_blockhash,
                fuzz_instructions,
            ));
        });
    }
}

async fn run_fuzz_instructions(
    token_vesting_testenv: &TokenVestingEnv,
    banks_client: &mut BanksClient,
    correct_payer: Keypair,
    recent_blockhash: Hash,
    fuzz_instructions: Vec<FuzzInstruction>,
) {
    // the reason we need a HashMap is because the fuzzer is generating u8 values - and we need Pubkeys/Keypairs
    // so we have to convert u8s -> into pubkeys/keypairs and store them
    let mut vesting_account_keys: HashMap<AccountId, Pubkey> = HashMap::new();
    let mut vesting_token_account_keys: HashMap<AccountId, Pubkey> = HashMap::new();
    let mut source_token_account_owner_keys: HashMap<AccountId, Keypair> = HashMap::new();
    let mut destination_token_owner_keys: HashMap<AccountId, Keypair> = HashMap::new();
    let mut destination_token_keys: HashMap<AccountId, Pubkey> = HashMap::new();
    let mut new_destination_token_keys: HashMap<AccountId, Pubkey> = HashMap::new();
    let mut mint_keys: HashMap<AccountId, Keypair> = HashMap::new();
    let mut payer_keys: HashMap<AccountId, Keypair> = HashMap::new();

    let mut global_output_ixs = vec![];
    let mut global_signer_keys = vec![];

    for ix in fuzz_instructions {
        vesting_account_keys
            .entry(ix.vesting_account_key)
            .or_insert_with(|| Pubkey::new_unique());
        vesting_token_account_keys
            .entry(ix.vesting_token_account_key)
            .or_insert_with(|| Pubkey::new_unique());
        source_token_account_owner_keys
            .entry(ix.source_token_account_owner_key)
            .or_insert_with(|| Keypair::new());
        destination_token_owner_keys
            .entry(ix.destination_token_owner_key)
            .or_insert_with(|| Keypair::new());
        destination_token_keys
            .entry(ix.destination_token_key)
            .or_insert_with(|| Pubkey::new_unique());
        new_destination_token_keys
            .entry(ix.new_destination_token_key)
            .or_insert_with(|| Pubkey::new_unique());
        mint_keys
            .entry(ix.mint_key)
            .or_insert_with(|| Keypair::new());
        payer_keys
            .entry(ix.payer_key)
            .or_insert_with(|| Keypair::new()); //this will be empty, no sol in it

        let (mut output_ix, mut signer_keys) = run_fuzz_ix(
            &token_vesting_testenv,
            &ix,
            &correct_payer,
            mint_keys.get(&ix.mint_key).unwrap(),
            vesting_account_keys.get(&ix.vesting_account_key).unwrap(),
            vesting_token_account_keys
                .get(&ix.vesting_token_account_key)
                .unwrap(),
            source_token_account_owner_keys
                .get(&ix.source_token_account_owner_key)
                .unwrap(),
            destination_token_owner_keys
                .get(&ix.destination_token_owner_key)
                .unwrap(),
            destination_token_keys
                .get(&ix.destination_token_key)
                .unwrap(),
            new_destination_token_keys
                .get(&ix.new_destination_token_key)
                .unwrap(),
            payer_keys.get(&ix.payer_key).unwrap(),
        );
        global_output_ixs.append(&mut output_ix);
        global_signer_keys.append(&mut signer_keys);
    }

    let mut tx = Transaction::new_with_payer(&global_output_ixs, Some(&correct_payer.pubkey()));
    let signers = [&correct_payer]
        .iter()
        .map(|&v| v) //needed to deref &Keypair
        .chain(global_signer_keys.iter())
        .collect::<Vec<&Keypair>>();
    tx.partial_sign(&signers, recent_blockhash);
    banks_client
        .process_transaction(tx)
        .await
        .unwrap_or_else(|e| {
            if let TransportError::TransactionError(te) = e {
                match te {
                    TransactionError::InstructionError(_, ie) => match ie {
                        InstructionError::InvalidArgument
                        | InstructionError::InvalidInstructionData
                        | InstructionError::InvalidAccountData
                        | InstructionError::InsufficientFunds
                        | InstructionError::AccountAlreadyInitialized
                        | InstructionError::InvalidSeeds
                        | InstructionError::Custom(0) => {}
                        _ => {
                            print!("{:?}", ie);
                            Err(ie).unwrap()
                        }
                    },
                    TransactionError::SignatureFailure
                    | TransactionError::InvalidAccountForFee
                    | TransactionError::InsufficientFundsForFee => {}
                    _ => {
                        print!("{:?}", te);
                        panic!()
                    }
                }
            } else {
                print!("{:?}", e);
                panic!()
            }
        });
}

fn run_fuzz_ix(
    token_vesting_testenv: &TokenVestingEnv,
    ix: &FuzzInstruction,
    correct_payer: &Keypair,
    mint_key: &Keypair,
    vesting_account_key: &Pubkey,
    vesting_token_account_key: &Pubkey,
    source_token_account_owner_key: &Keypair,
    destination_token_owner_key: &Keypair,
    destination_token_key: &Pubkey,
    new_destination_token_key: &Pubkey,
    payer_key: &Keypair,
) -> (Vec<Instruction>, Vec<Keypair>) {
    // basically, depending on the boolean generated by the fuzzer, we can decide to try to run an tx with correct inputs or with wrong inputs
    if ix.correct_inputs {
        //if we decide to run a correct tx, we first need to fix some inputs
        //we use the seeds to derive a real PDA account, and then update the seeds to it captures the bump
        let mut correct_seeds = ix.seeds;
        let (correct_vesting_account_key, bump) = Pubkey::find_program_address(
            &[&correct_seeds[..31]], //take 31 out of 32 bytes to generate the bump
            &token_vesting_testenv.vesting_program_id,
        );
        correct_seeds[31] = bump; //assign that bump as 32nd byte into the array. now this array represents the entire seed used to derive the vesting account
                                  // from vesting account generate vesting token account
        let correct_vesting_token_key =
            get_associated_token_address(&correct_vesting_account_key, &mint_key.pubkey());
        // also separately generate the source token account
        let correct_source_token_account_key = get_associated_token_address(
            &source_token_account_owner_key.pubkey(),
            &mint_key.pubkey(),
        );

        // only then we proceed with matching, with correct inputs

        match ix {
            // -----------------------------------------------------------------------------
            FuzzInstruction {
                instruction: VestingInstruction::Init { .. },
                ..
            } => {
                let init_ix = init(
                    &token_vesting_testenv.system_program_id,
                    &token_vesting_testenv.rent_program_id,
                    &token_vesting_testenv.vesting_program_id,
                    &correct_payer.pubkey(), //correct in a sense that it's the payer account generated for us by the test program and so it actually has sol in it
                    &correct_vesting_account_key,
                    correct_seeds,
                    ix.number_of_schedules as u32,
                ).unwrap();
                let ix_vec = vec![init_ix];
                let kp_vec = vec![];
                return (ix_vec, kp_vec);
            }
            // -----------------------------------------------------------------------------
            FuzzInstruction {
                instruction: VestingInstruction::Create { ..},
                ..
            } => {
                let init_ix = init(
                    &token_vesting_testenv.system_program_id,
                    &token_vesting_testenv.rent_program_id,
                    &token_vesting_testenv.vesting_program_id,
                    &correct_payer.pubkey(), //correct in a sense that it's the payer account generated for us by the test program and so it actually has sol in it
                    &correct_vesting_account_key,
                    correct_seeds,
                    ix.number_of_schedules as u32,
                ).unwrap();
                let mut create_instructions = create_fuzzinstruction(
                        token_vesting_testenv,
                        ix,
                        correct_payer,
                        &correct_source_token_account_key,
                        source_token_account_owner_key,
                        destination_token_key,
                        &destination_token_owner_key.pubkey(),
                        &correct_vesting_account_key,
                        &correct_vesting_token_key,
                        correct_seeds,
                        mint_key,
                        ix.source_token_amount);
                let mut ix_vec = vec![init_ix];
                ix_vec.append(&mut create_instructions);
                let kp_vec = vec![
                        clone_keypair(mint_key),
                        clone_keypair(&token_vesting_testenv.mint_authority_keypair),
                        clone_keypair(source_token_account_owner_key)];
                return (ix_vec, kp_vec);
            }
            // -----------------------------------------------------------------------------
            // unlock = basically everything in create + unlock() on top
            // -----------------------------------------------------------------------------
            // change also nothing new, not doing
            // -----------------------------------------------------------------------------
            FuzzInstruction {
                instruction: VestingInstruction::Empty { .. }, // ignore what's the actual argument passed into init - we don't care at this stage
                .. //ignore all the other fields in the struct - so we're only matching on one
            } => {
                let empty_ix = prepare_dummy_empty_ix(token_vesting_testenv.vesting_program_id);
                let ix_vec = vec![empty_ix];
                let kp_vec = vec![];
                return (ix_vec, kp_vec);
            }
            _ => {
                return (vec![], vec![]);
            }
        }
    //otherwise, if we don't want a correc tx, we go ahead with existing inputs
    //why do this? because we're actually catching these errors in unwrap_or_else() above, and printing them out instead of panicking
    //the only times we panic is when we get UNEXPECTED erros. that's why this is powerful.
    } else {
        match ix {
            FuzzInstruction {
                instruction: VestingInstruction::Init { .. },
                ..
            } => {
                let init_ix = init(
                    &token_vesting_testenv.system_program_id,
                    &token_vesting_testenv.rent_program_id,
                    &token_vesting_testenv.vesting_program_id,
                    &payer_key.pubkey(), //we're using a pubkey with no sol in the address
                    vesting_account_key, //we're using a vesting account that wasn't actually derived from the vesting program - and so one of the checks in the contract will fail
                    ix.seeds,
                    ix.number_of_schedules as u32,
                )
                .unwrap();
                let ix_vec = vec![init_ix];
                let kp_vec = vec![];
                return (ix_vec, kp_vec);
            }
            _ => {
                return (vec![], vec![]);
            }
        }
    }
}

fn prepare_dummy_empty_ix(program_id: Pubkey) -> Instruction {
    let mut z = vec![4_u8];
    let x = 32_u32.to_le_bytes();
    z.extend(&x);
    Instruction::new_with_bytes(program_id, &z, vec![])
}

// A correct vesting create fuzz instruction
fn create_fuzzinstruction(
    token_vesting_testenv: &TokenVestingEnv,
    fuzz_instruction: &FuzzInstruction,
    payer: &Keypair,
    correct_source_token_account_key: &Pubkey,
    source_token_account_owner_key: &Keypair,
    destination_token_key: &Pubkey,
    destination_token_owner_key: &Pubkey,
    correct_vesting_account_key: &Pubkey,
    correct_vesting_token_key: &Pubkey,
    correct_seeds: [u8; 32],
    mint_key: &Keypair,
    source_amount: u64,
) -> Vec<Instruction> {
    // Initialize the token mint account
    let mut instructions_acc = mint_init_instruction(
        &payer,
        &mint_key,
        &token_vesting_testenv.mint_authority_keypair,
    );

    // Create the associated token accounts
    let source_instruction = create_associated_token_account(
        &payer.pubkey(),
        &source_token_account_owner_key.pubkey(),
        &mint_key.pubkey(),
    );
    instructions_acc.push(source_instruction);

    let vesting_instruction = create_associated_token_account(
        &payer.pubkey(),
        &correct_vesting_account_key,
        &mint_key.pubkey(),
    );
    instructions_acc.push(vesting_instruction);

    let destination_instruction = create_associated_token_account(
        &payer.pubkey(),
        &destination_token_owner_key,
        &mint_key.pubkey(),
    );
    instructions_acc.push(destination_instruction);

    // Credit the source account
    let setup_instruction = mint_to(
        &spl_token::id(),
        &mint_key.pubkey(),
        &correct_source_token_account_key,
        &token_vesting_testenv.mint_authority_keypair.pubkey(),
        &[],
        source_amount,
    )
    .unwrap();
    instructions_acc.push(setup_instruction);

    let used_number_of_schedules = fuzz_instruction.number_of_schedules.min(
        fuzz_instruction
            .schedules
            .len()
            .try_into()
            .unwrap_or(u8::MAX),
    );
    // Initialize the vesting program account
    let create_instruction = create(
        &token_vesting_testenv.vesting_program_id,
        &token_vesting_testenv.token_program_id,
        &correct_vesting_account_key,
        &correct_vesting_token_key,
        &source_token_account_owner_key.pubkey(),
        &correct_source_token_account_key,
        &destination_token_key,
        &mint_key.pubkey(),
        fuzz_instruction.schedules.clone()[..used_number_of_schedules.into()].into(),
        correct_seeds,
    )
    .unwrap();
    instructions_acc.push(create_instruction);

    return instructions_acc;
}

// Helper functions
fn mint_init_instruction(
    payer: &Keypair,
    mint: &Keypair,
    mint_authority: &Keypair,
) -> Vec<Instruction> {
    let instructions = vec![
        system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            Rent::default().minimum_balance(82),
            82,
            &spl_token::id(),
        ),
        initialize_mint(
            &spl_token::id(),
            &mint.pubkey(),
            &mint_authority.pubkey(),
            None,
            0,
        )
        .unwrap(),
    ];
    return instructions;
}

fn clone_keypair(keypair: &Keypair) -> Keypair {
    return Keypair::from_bytes(&keypair.to_bytes().clone()).unwrap();
}

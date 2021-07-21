#![no_main]
use std::{borrow::Borrow, convert::TryInto, str::FromStr};

use libfuzzer_sys::fuzz_target;
use rebuild_rs::{
    instruction::{create, unlock, Schedule, VestingInstruction},
    processor::Processor,
    state::VestingSchedule,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::create_account,
    system_program,
    sysvar::{self},
};
use solana_program_test::*;
use solana_sdk::{
    hash::Hash,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::solana_program::program_pack::Pack;

// ----------------------------------------------------------------------------- structs / consts

const SEED: &str = "11111111yayayayayyayayayayyayayayayyayayayayyayayayay";

pub struct TokenVestingEnv {}

#[derive(Debug, arbitrary::Arbitrary, Clone)]
pub struct FuzzInstruction {
    pub amount: u64,
}

// ----------------------------------------------------------------------------- fuzz_target

fuzz_target!(|fuzz_instruction: FuzzInstruction| {
    // println!("amount is {}", fuzz_instruction.amount);
    // assert!(fuzz_instruction.amount > 1111111);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        test_init_create_unlock_flow().await;
    })
});

// ----------------------------------------------------------------------------- helpers

async fn setup_test_env() -> (BanksClient, Keypair, Hash, Pubkey) {
    let program_id = Pubkey::from_str("SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM").unwrap();
    let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
        "rebuild_rs", //must match crate name or cargo test-bpf won't work
        program_id,
        processor!(Processor::process_instruction),
    )
    .start()
    .await;

    (banks_client, payer, recent_blockhash, program_id)
}

// #[tokio::test]
async fn test_empty_ix() {
    let (mut banks_client, payer, recent_blockhash, program_id) = setup_test_env().await;

    // ----------------------------------------------------------------------------- 1a manual
    // let z = vec![4_u8, 4, 0, 0, 0];
    // let mut tx = Transaction::new_with_payer(
    //     &[Instruction::new_with_bytes(program_id, &z2, vec![])],
    //     Some(&payer.pubkey()),
    // );

    // ----------------------------------------------------------------------------- 1a semi-manual
    let mut z = vec![4_u8];
    let x = 32_u32.to_le_bytes();
    z.extend(&x);
    let mut tx = Transaction::new_with_payer(
        &[Instruction::new_with_bytes(program_id, &z, vec![])],
        Some(&payer.pubkey()),
    );

    // ----------------------------------------------------------------------------- 2 automatic - bincode
    // requires deserialization with bincode on the other side

    // let mut tx = Transaction::new_with_payer(
    //     &[Instruction::new_with_bincode(
    //         program_id,
    //         &VestingInstruction::Empty { number: 5 },
    //         vec![],
    //     )],
    //     Some(&payer.pubkey()),
    // );

    // ----------------------------------------------------------------------------- 3 automatic - borsh
    // (!) requires deserialization with borsh on the other side

    // let mut tx = Transaction::new_with_payer(
    //     &[Instruction::new_with_borsh(
    //         program_id,
    //         &VestingInstruction::Empty { number: 5 },
    //         vec![],
    //     )],
    //     Some(&payer.pubkey()),
    // );

    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();
}

// #[tokio::test]
async fn test_init_create_unlock_flow() {
    let (mut banks_client, payer, recent_blockhash, program_id) = setup_test_env().await;

    // ----------------------------------------------------------------------------- 1 call init

    // LOL I packed the data here manually, but actually there wasn't any need for this - I could have just used pub fn init() from instruction.rs
    let mut data = vec![0_u8];
    let num_schedules = 1_u32.to_le_bytes();
    data.extend(&*SEED[..32].as_bytes());
    data.extend(&num_schedules);

    let vesting_account_key =
        Pubkey::create_program_address(&[&SEED[..32].as_bytes()], &program_id).unwrap();

    let mut tx = Transaction::new_with_payer(
        &[Instruction::new_with_bytes(
            program_id,
            &data,
            vec![
                //   0. `[]` The system program account
                AccountMeta::new_readonly(system_program::id(), false),
                //   1. `[]` The sysvar Rent account
                AccountMeta::new_readonly(sysvar::rent::id(), false),
                //   1. `[signer]` The fee payer account
                AccountMeta::new_readonly(payer.pubkey(), true),
                //   1. `[writable]` The vesting account
                AccountMeta::new(vesting_account_key, false),
            ],
        )],
        Some(&payer.pubkey()),
    );

    tx.sign(&[&payer], recent_blockhash);
    //in a sense this .unwrap() is the first assert!()
    //if there was any error while executing the contract, this would also throw an error
    banks_client.process_transaction(tx).await.unwrap();

    // ----------------------------------------------------------------------------- 2 interm step - create assoc token acc

    // step 1 - we need to create a new token. We can't use existing because the payer, which is randomly derived in this test, needs to have the right to mint tokens
    // 1.1 we'll need a new keypair
    let mint_keypair = solana_sdk::signature::Keypair::new();

    // 1.2 so that we can create a new account
    let rent = banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);
    let create_account_ix = create_account(
        &payer.pubkey(),
        &mint_keypair.pubkey(),
        mint_rent,
        spl_token::state::Mint::LEN as u64,
        &spl_token::id(), //we're making the spl_token the owner
    );

    // 1.3 which we will initialize as token mint account
    let init_token_mint_acc_ix = spl_token::instruction::initialize_mint(
        &spl_token::id(),
        &mint_keypair.pubkey(),
        &payer.pubkey(),
        Some(&payer.pubkey()),
        0,
    )
    .unwrap();

    let mut create_token_tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_token_mint_acc_ix],
        Some(&payer.pubkey()),
        &[&payer, &mint_keypair], //&[&b"escrow"[..], &[bump_seed]]
        recent_blockhash,
    );
    banks_client
        .process_transaction(create_token_tx)
        .await
        .unwrap();

    // step 2 - create an associated token account
    // this consists of 2 sub-steps:
    // step 2.1: we find the associated address, because we're going to pass it in - https://docs.rs/spl-associated-token-account/1.0.2/spl_associated_token_account/fn.get_associated_token_address.html
    // - note that the wallet address is the vesting_account, because we want the vesting_token_account to be owned by the vesting_account

    let vesting_token_account_key = spl_associated_token_account::get_associated_token_address(
        &vesting_account_key,
        &mint_keypair.pubkey(),
    );

    // step 2.2: we issue a call to create that address to the associated token program - https://docs.rs/spl-associated-token-account/1.0.2/spl_associated_token_account/fn.create_associated_token_account.html
    // - note that the program only has 1 instruction, which is why we don't need to send any data. we only need to pass in the right accounts
    let accounts = vec![
        //   pubkey: payer, isSigner, isWritable
        AccountMeta::new(payer.pubkey(), true),
        //   pubkey: vesting_token_account, isWritable
        AccountMeta::new(vesting_token_account_key, false),
        //   pubkey: vesting_account,
        AccountMeta::new_readonly(vesting_account_key, false),
        //   pubkey: splTokenMintAddress,
        AccountMeta::new_readonly(mint_keypair.pubkey(), false),
        //   pubkey: systemProgramId,
        AccountMeta::new_readonly(system_program::id(), false),
        //   pubkey: TOKEN_PROGRAM_ID,
        AccountMeta::new_readonly(spl_token::id(), false),
        //   pubkey: SYSVAR_RENT_PUBKEY,
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    let mut token_tx = Transaction::new_with_payer(
        &[Instruction::new_with_bytes(
            spl_associated_token_account::id(),
            &[], //no data because this program only executes 1 instruction
            accounts,
        )],
        Some(&payer.pubkey()),
    );

    // (!) COULD HAVE JUST USED THE BELOW, BUT WOULD STILL NEED TO RUN "GET" AS WE NEED THE ADDR FURTHER
    // let ix_to_create_assoc_acc = spl_associated_token_account::create_associated_token_account(
    //     &payer.pubkey(),
    //     &vesting_account_key,
    //     &mint_keypair.pubkey(),
    // );
    //
    // println!("ix is: {:?}", ix_to_create_assoc_acc);

    // let mut token_tx =
    //     Transaction::new_with_payer(&[ix_to_create_assoc_acc], Some(&payer.pubkey()));

    token_tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(token_tx).await.unwrap();

    // ----------------------------------------------------------------------------- 3 create source & mint some tokens

    // create an associated token account from main payer's account
    let create_source_token_acc_ix = spl_associated_token_account::create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &mint_keypair.pubkey(),
    );

    // get the key so that we can use it minter ix below
    let source_token_acc_key = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &mint_keypair.pubkey(),
    );

    //note how this time we're using a helper function instead of manually building up the tx data
    let mint_to_source_acc_ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        &mint_keypair.pubkey(),
        &source_token_acc_key,
        &payer.pubkey(),
        &[&payer.pubkey()],
        1000,
    )
    .unwrap();

    let mint_tx = Transaction::new_signed_with_payer(
        &[create_source_token_acc_ix, mint_to_source_acc_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(mint_tx).await.unwrap();

    // ----------------------------------------------------------------------------- 4 create dest & call create

    let dest_keypair = solana_sdk::signature::Keypair::new();

    // let create_dest_acc_ix = solana_program::system_instruction::create_account(
    //     &dest_keypair
    // )

    let create_dest_token_acc_ix = spl_associated_token_account::create_associated_token_account(
        &payer.pubkey(),
        &dest_keypair.pubkey(),
        &mint_keypair.pubkey(),
    );

    let dest_token_acc_key = spl_associated_token_account::get_associated_token_address(
        &dest_keypair.pubkey(),
        &mint_keypair.pubkey(),
    );

    let s = rebuild_rs::instruction::Schedule {
        release_time: 1,
        amount: 111,
    };
    let schedules = vec![s];

    // try_into() instead of into() because forcing an arb-sized array into a fixed size might fail
    // https://users.rust-lang.org/t/why-from-u8-is-not-implemented-for-u8-x/35590
    let seeds: [u8; 32] = (&*SEED[..32].as_bytes()).try_into().unwrap();

    let create_vesting_contract_ix = create(
        &program_id,
        &spl_token::id(),
        &vesting_account_key,
        &vesting_token_account_key,
        &payer.pubkey(),
        &source_token_acc_key,
        &dest_token_acc_key,
        &mint_keypair.pubkey(),
        schedules,
        seeds,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_dest_token_acc_ix, create_vesting_contract_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // ----------------------------------------------------------------------------- 5 test unlock

    let unlock_contract_ix = unlock(
        &program_id,
        &spl_token::id(),
        &sysvar::clock::id(),
        &vesting_account_key,
        &vesting_token_account_key,
        &dest_token_acc_key,
        seeds,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[unlock_contract_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // ----------------------------------------------------------------------------- verify state on the blockchain

    // let client = solana_client::rpc_client::RpcClient::new("http://localhost:8899".into());
    // let dest_acc = client.get_account(&dest_token_acc_key).unwrap();

    let dest_acc = banks_client
        .get_account(dest_token_acc_key)
        .await
        .unwrap()
        .unwrap();
    let dest_token_acc_state = spl_token::state::Account::unpack(&dest_acc.data.borrow()).unwrap();
    assert_eq!(dest_token_acc_state.amount, 111);

    let source_acc = banks_client
        .get_account(source_token_acc_key)
        .await
        .unwrap()
        .unwrap();
    let source_token_acc_state =
        spl_token::state::Account::unpack_from_slice(&source_acc.data).unwrap();
    assert_eq!(source_token_acc_state.amount, 1000 - 111);

    println!("it workerd");
}

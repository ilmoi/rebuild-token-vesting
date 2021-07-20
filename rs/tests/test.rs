use rebuild_rs::instruction::VestingInstruction;
use {
    rebuild_rs::processor::Processor,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
        sysvar::{self},
    },
    solana_program_test::*,
    solana_sdk::{signature::Signer, transaction::Transaction},
    std::str::FromStr,
};

#[tokio::test]
async fn test_empty_ix() {
    let program_id = Pubkey::from_str("SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM").unwrap();
    let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
        "token_vesting",
        program_id,
        processor!(Processor::process_instruction),
    )
    .start()
    .await;

    // ----------------------------------------------------------------------------- 1a manual
    // let z = vec![4_u8, 4, 0, 0, 0];
    // let mut tx = Transaction::new_with_payer(
    //     &[Instruction::new_with_bytes(program_id, &z2, vec![])],
    //     Some(&payer.pubkey()),
    // );

    // ----------------------------------------------------------------------------- 1a semi-manual
    // let mut z = vec![4_u8];
    // let x = 32_u32.to_le_bytes();
    // z.extend(&x);
    // let mut tx = Transaction::new_with_payer(
    //     &[Instruction::new_with_bytes(program_id, &z, vec![])],
    //     Some(&payer.pubkey()),
    // );

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

    let mut tx = Transaction::new_with_payer(
        &[Instruction::new_with_borsh(
            program_id,
            &VestingInstruction::Empty { number: 5 },
            vec![],
        )],
        Some(&payer.pubkey()),
    );

    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();
}

#[tokio::test]
async fn test_init_ix() {
    let program_id = Pubkey::from_str("SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM").unwrap();
    let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
        "token_vesting",
        program_id,
        processor!(Processor::process_instruction),
    )
    .start()
    .await;

    let mut data = vec![0_u8];
    let seed = &"11111111yayayayayyayayayayyayayayayyayayayayyayayayay".as_bytes()[..32];
    let num_schedules = 1_u32.to_le_bytes();
    data.extend(seed);
    data.extend(&num_schedules);
    println!("data len is {}", data.len());
    println!("data is {:?}", data);

    let mut tx = Transaction::new_with_payer(
        &[Instruction::new_with_bytes(
            program_id,
            &data,
            vec![
                ///   0. `[]` The system program account
                AccountMeta::new(system_program::id(), false),
                ///   1. `[]` The sysvar Rent account
                AccountMeta::new(sysvar::rent::id(), false),
                ///   1. `[signer]` The fee payer account
                AccountMeta::new(payer.pubkey(), true),
                ///   1. `[]` The vesting account
                AccountMeta::new(Pubkey::new_unique(), false),
            ],
        )],
        Some(&payer.pubkey()),
    );

    // todo deserializing 255?

    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();
}

#![no_main]
use libfuzzer_sys::fuzz_target;
use {
    solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        sysvar::{self},
    },
    solana_program_test::*,
    solana_sdk::{signature::Signer, transaction::Transaction},
    spl_example_sysvar::processor::process_instruction,
    std::str::FromStr,
};

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
});

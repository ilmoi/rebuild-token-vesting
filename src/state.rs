use solana_program::pubkey::Pubkey;
use solana_program::program_pack::{Sealed, IsInitialized, Pack};
use solana_program::msg;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::program_error::ProgramError;

#[derive(Debug, PartialEq)]
pub struct VestingSchedule {
    pub release_time: u64,
    pub amount: u64,
}

#[derive(Debug, PartialEq)]
pub struct VestingScheduleHeader {
    pub destination_address: Pubkey,
    pub mint_address: Pubkey,
    pub is_initialized: bool,
}

// https://docs.rs/solana-program/1.7.4/solana_program/program_pack/index.html
// there are 3 standard traits that we have to define as per program_pack module:
// 1)is_initialized = check if state has been initialized
// 2)pack = de/serialize state
// 3)sealed = solana's version of size


// ----------------------------------------------------------------------------- 1)
// just take the default implementation
impl Sealed for VestingSchedule {}
impl Sealed for VestingScheduleHeader {}

// ----------------------------------------------------------------------------- 2)
// interesting, so you DONT HAVE TO implement it for each struct... the Bonfida guys didnt impl for the second one
impl IsInitialized for VestingScheduleHeader {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

// ----------------------------------------------------------------------------- 3)
impl Pack for VestingSchedule {
    const LEN: usize = 16;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref!(dst, 0, VestingSchedule::LEN); //gen mutable ref to a subset of a slice

        // prepare the byte slices we'll be filling in
        let (
            dst_release_time,
            dst_amount,
        ) = mut_array_refs![dst, 8, 8]; //get multiple mutable refs to subsets of a slice

        // fill in the byte fields from self
        *dst_release_time = self.release_time.to_le_bytes(); //todo weird - little endian not big...
        *dst_amount = self.release_time.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        if src.len() < 16 {
            msg!("passed slice is shorter than 16 bytes");
            return Err(ProgramError::InvalidAccountData);
        }

        let src = array_ref!(src, 0, VestingSchedule::LEN); //gen an array ref to a subset of a slice

        // get refs to each slice we're interested in
        let (
            src_release_time,
            src_amount,
        ) = array_refs![src, 8, 8]; //get multiple refs to multiple subsets of a slice

        Ok(Self {
            release_time: u64::from_le_bytes(*src_release_time),
            amount: u64::from_le_bytes(*src_amount),
        })
    }
}

impl Pack for VestingScheduleHeader {
    //each pubkey = 32x2 + bool
    const LEN: usize = 65;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref!(dst, 0, VestingScheduleHeader::LEN); //gen mutable ref to a subset of a slice

        // prepare the byte slices we'll be filling in
        let (
            dst_destination_address,
            dst_mint_address,
            dst_is_initialized,
        ) = mut_array_refs![dst, 32, 32, 1]; //get multiple mutable refs to subsets of a slice

        // fill in the byte fields from self
        dst_destination_address.copy_from_slice(self.destination_address.as_ref());
        dst_mint_address.copy_from_slice(self.mint_address.as_ref());
        dst_is_initialized[0] = self.is_initialized as u8;
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        if src.len() < 65 {
            msg!("passed slice is shorter than 65 bytes");
            return Err(ProgramError::InvalidAccountData);
        }

        let src = array_ref!(src, 0, VestingScheduleHeader::LEN); //gen an array ref to a subset of a slice

        // get refs to each slice we're interested in
        let (
            src_destination_address,
            src_mint_address,
            src_is_initialized,
        ) = array_refs![src, 32, 32, 1]; //get multiple refs to multiple subsets of a slice

        let is_initialized = match src_is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        Ok(Self {
            destination_address: Pubkey::new_from_array(*src_destination_address),
            mint_address: Pubkey::new_from_array(*src_mint_address),
            is_initialized,
        })
    }
}

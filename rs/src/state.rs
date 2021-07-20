use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

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
        let (dst_release_time, dst_amount) = mut_array_refs![dst, 8, 8]; //get multiple mutable refs to subsets of a slice

        // fill in the byte fields from self
        *dst_release_time = self.release_time.to_le_bytes();
        *dst_amount = self.amount.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        if src.len() < 16 {
            msg!("passed slice is shorter than 16 bytes");
            return Err(ProgramError::InvalidAccountData);
        }

        let src = array_ref!(src, 0, VestingSchedule::LEN); //gen an array ref to a subset of a slice

        // get refs to each slice we're interested in
        let (src_release_time, src_amount) = array_refs![src, 8, 8]; //get multiple refs to multiple subsets of a slice

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
        let (dst_destination_address, dst_mint_address, dst_is_initialized) =
            mut_array_refs![dst, 32, 32, 1]; //get multiple mutable refs to subsets of a slice

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
        let (src_destination_address, src_mint_address, src_is_initialized) =
            array_refs![src, 32, 32, 1]; //get multiple refs to multiple subsets of a slice

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

// ----------------------------------------------------------------------------- other

pub fn unpack_schedules(input: &[u8]) -> Result<Vec<VestingSchedule>, ProgramError> {
    let number_of_schedules = input.len() / VestingSchedule::LEN;
    let mut output: Vec<VestingSchedule> = Vec::with_capacity(number_of_schedules);
    let mut offset = 0;
    for _ in 0..number_of_schedules {
        output.push(VestingSchedule::unpack_from_slice(
            &input[offset..offset + VestingSchedule::LEN],
        )?);
        offset += VestingSchedule::LEN;
    }
    Ok(output)
}

pub fn pack_schedules_into_slice(schedules: Vec<VestingSchedule>, target: &mut [u8]) {
    let mut offset = 0;
    for s in schedules.iter() {
        s.pack_into_slice(&mut target[offset..]);
        offset += VestingSchedule::LEN;
    }
}

// ----------------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_packing() {
        // create the 3 structures
        let header = VestingScheduleHeader {
            destination_address: Pubkey::new_unique(), //nice function for testing
            mint_address: Pubkey::new_unique(),
            is_initialized: true,
        };
        let schedule_1 = VestingSchedule {
            release_time: 1,
            amount: 333,
        };
        let schedule_2 = VestingSchedule {
            release_time: 99999,
            amount: 111,
        };

        // pack them into a slice
        const SIZE: usize = VestingScheduleHeader::LEN + 2 * VestingSchedule::LEN;
        let mut state_array = [0_u8; SIZE];

        header.pack_into_slice(&mut state_array[..VestingScheduleHeader::LEN]);
        schedule_1.pack_into_slice(
            &mut state_array
                [VestingScheduleHeader::LEN..VestingScheduleHeader::LEN + VestingSchedule::LEN],
        );
        schedule_2
            .pack_into_slice(&mut state_array[VestingScheduleHeader::LEN + VestingSchedule::LEN..]);
        let packed = Vec::from(state_array);

        // create an empty vector of same size
        let mut expected = Vec::<u8>::with_capacity(SIZE);
        // use extend_from_slice and to_le_bytes() to pack it
        expected.extend_from_slice(&header.destination_address.to_bytes());
        expected.extend_from_slice(&header.mint_address.to_bytes());
        expected.extend_from_slice(&[header.is_initialized as u8]);
        expected.extend_from_slice(&schedule_1.release_time.to_le_bytes());
        expected.extend_from_slice(&schedule_1.amount.to_le_bytes());
        expected.extend_from_slice(&schedule_2.release_time.to_le_bytes());
        expected.extend_from_slice(&schedule_2.amount.to_le_bytes());

        // test packing
        assert_eq!(packed, expected);

        // test unpacking
        let unpacked_header =
            VestingScheduleHeader::unpack_from_slice(&packed[..VestingScheduleHeader::LEN])
                .unwrap();
        assert_eq!(header, unpacked_header);
        let unpacked_s1 = VestingSchedule::unpack_from_slice(
            &packed[VestingScheduleHeader::LEN..VestingScheduleHeader::LEN + VestingSchedule::LEN],
        )
        .unwrap();
        assert_eq!(schedule_1, unpacked_s1);
        let unpacked_s2 = VestingSchedule::unpack_from_slice(
            &packed[VestingScheduleHeader::LEN + VestingSchedule::LEN..],
        )
        .unwrap();
        assert_eq!(schedule_2, unpacked_s2);
    }
}

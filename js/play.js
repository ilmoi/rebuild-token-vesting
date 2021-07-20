const {Numberu64} = require("@bonfida/token-vesting");
const {create, Schedule} = require("@bonfida/token-vesting");
const {TOKEN_PROGRAM_ID, Token} = require('@solana/spl-token');
const {Account, PublicKey, Connection, sendAndConfirmTransaction} = require('@solana/web3.js');
const BN = require('bn.js');
const {TransactionInstruction} = require("@solana/web3.js");
const {unlock, changeDestination} = require("@bonfida/token-vesting");
const {findAssociatedTokenAddress} = require("@bonfida/token-vesting");
const {Transaction} = require("@solana/web3.js");
const borsh = require('borsh');

const connection = new Connection('http://localhost:8899', 'processed');

const privateKeyByteArray =
  '201,101,147,128,138,189,70,190,202,49,28,26,32,21,104,185,191,41,20,171,3,144,4,26,169,73,180,171,71,22,48,135,231,91,179,215,3,117,187,183,96,74,154,155,197,243,114,104,20,123,105,47,181,123,171,133,73,181,102,41,236,78,210,176';
const privateKeyDecoded = privateKeyByteArray.split(',').map(s => parseInt(s));
const payer = new Account(privateKeyDecoded);

const bobPrivateKeyByteArray =
  '177,217,193,155,63,150,164,184,81,82,121,165,202,87,86,237,218,226,212,201,167,170,149,183,59,43,155,112,189,239,231,110,162,218,184,20,108,2,92,114,203,184,223,69,137,206,102,71,162,0,127,63,170,96,137,108,228,31,181,113,57,189,30,76';
const bobPrivateKeyDecoded = bobPrivateKeyByteArray.split(',').map(s => parseInt(s));
const bob = new Account(bobPrivateKeyDecoded);

const vesting_program_id = new PublicKey('SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM');
const minter_pk = new PublicKey('5e48G9KL813hkT9LRCHs6uGFdrhAihP8Jbk1QfScE78R');
const alice_pk = new PublicKey('6u1TUSDgHbQsMk33vuPF61yv6k7E2fxm9FYufuEGs1SU'); //holds 999 tokens
const bob_pk = new PublicKey('CJrc1GzvC18tzrpNrYdVriwxmY4fC5RJyF3EVB2CoJVU'); //holds 1

// ----------------------------------------------------------------------------- create

let now = Math.floor(Date.now() / 1000); //unix timestamp in SECONDS
now = now - 1000000; //unix timestamp in milliseconds
// const ten_min_from_nom = now + 10 * 60 * 1000;

console.log('TIME IS: ', now);

// needs to be BIG endinan not little, because Numberu64::fromBuffer() is calling .reverse() inside of it
const now_buffer = Buffer.from(Uint8Array.of(...new BN(now).toArray("be", 8)))
const amount_buffer = Buffer.from(Uint8Array.of(...new BN(50).toArray("be", 8)))
// console.log(now_buffer);
// console.log(amount_buffer);

const now_u64 = Numberu64.fromBuffer(now_buffer);
const amount_u64 = Numberu64.fromBuffer(amount_buffer);

const schedules = [
  new Schedule(now_u64, amount_u64),
]

const seed = Buffer.from("31111111yayayayayyayayayayyayayayayyayayayayyayayayay"); //had to be long enough, at least 32 bytes

async function createContract() {
  const create_ix = await create(
    connection,
    vesting_program_id,
    seed, //used to derive vestingAccountKey
    payer.publicKey,
    payer.publicKey,
    null, //this gets derived automatically
    bob_pk,
    minter_pk,
    schedules,
  )
  // console.log(create_ix)

  const tx = new Transaction().add(create_ix[0], create_ix[1], create_ix[2]);
  let tx_hash = await sendAndConfirmTransaction(connection, tx, [payer])
  console.log(tx_hash)
}

// ----------------------------------------------------------------------------- unlock

async function unlockContract() {
  const unlock_ix = await unlock(
    connection,
    vesting_program_id,
    seed, //used to derive vestingAccountKey
    minter_pk,
  )
  const tx = new Transaction().add(unlock_ix[0]);
  let tx_hash = await sendAndConfirmTransaction(connection, tx, [payer])
  console.log(tx_hash)

}

// ----------------------------------------------------------------------------- change dest

async function changeDest() {
  const change_ix = await changeDestination(
    connection,
    vesting_program_id,
    bob.publicKey,
    payer.publicKey,
    alice_pk,
    [seed],
  )
  const tx = new Transaction().add(change_ix[0]);
  let tx_hash = await sendAndConfirmTransaction(connection, tx, [bob])
  console.log(tx_hash)
}

// ----------------------------------------------------------------------------- empty

async function callEmpty() {

  // ----------------------------------------------------------------------------- 1 manual
  const data = Buffer.from(Uint8Array.of(4, ...new BN(5).toArray("le",4)));

  // ----------------------------------------------------------------------------- 2 bincode
  // didn't bother writing a proper serializer because solana docs say it's expensive to use program-side
  // if I ever do this, this might help - https://github.com/timfish/bincode-typescript

  // const data = Buffer.from(Uint8Array.of(4, 0, 0, 0, ...new BN(5).toArray("le",4)));

//   // ----------------------------------------------------------------------------- 3 borsh
//   // discussed here - https://github.com/near/borsh-js/issues/21
//
//   // ------------------------------step 1: class
//
//   class Empty {
//     number = 0;
//
//     constructor(fields) {
//       if (fields) {
//         this.number = fields.number;
//       }
//     }
//   }
//
//   const empty_schema = new Map([[Empty, {kind: 'struct', fields: [['number', 'u32']]}]]);
//   const empty_size = borsh.serialize(empty_schema, new Empty()).length;
//
//   const empty_serialized = borsh.serialize(empty_schema, new Empty({number: 5}));
//
//   // ------------------------------step 2a: hacking way
//   //   class Ix {
//   //     data = 'serialized_data';
//   //
//   //     constructor(fields) {
//   //       if (fields) {
//   //         this.serialized_data = fields.serialized_data;
//   //       }
//   //     }
//   //   }
//   //
//   //   const ixx = new Ix({serialized_data: empty_serialized});
//   //
//   //   const values = [
//   //     [],
//   //     [],
//   //     [],
//   //     [],
//   //     ['serialized_data', [empty_serialized.length]],
//   //   ];
//   //
//   //   const schema = new Map([[Ix, {kind: 'enum', field: 'data', values: values}]]);
//   //   const data = borsh.serialize(schema, ixx);
//   //
//   //   const ix = new TransactionInstruction(
//   //     {
//   //       keys: [],
//   //       programId: vesting_program_id,
//   //       data,
//   //     }
//   //   )
//   //   const tx = new Transaction().add(ix);
//   //   let tx_hash = await sendAndConfirmTransaction(connection, tx, [payer])
//   //   console.log(tx_hash)
//   // }
//
//   // ------------------------------step 2b: less hacking way
//   class Ix {
//     init = 'serialized_init';
//     create = 'serialized_create';
//     unlock = 'serialized_unlock';
//     changed = 'serialized_changed';
//     empty = 'serialized_empty';
//
//     constructor(fields) {
//       if (fields) {
//         this.serialized_init = fields.serialized_init;
//         this.serialized_create = fields.serialized_create;
//         this.serialized_unlock = fields.serialized_unlock;
//         this.serialized_changed = fields.serialized_changed;
//         this.serialized_empty = fields.serialized_empty;
//       }
//     }
//   }
//
//   const ixx = new Ix({serialized_empty: empty_serialized});
//
//   const values = [
//     ['serialized_init', [1]],
//     ['serialized_create', [1]],
//     ['serialized_unlock', [1]],
//     ['serialized_changed', [1]],
//     ['serialized_empty', [empty_size]]
//   ];
//
//   const schema = new Map([[Ix, {kind: 'enum', field: 'empty', values: values}]]);
//   const data = borsh.serialize(schema, ixx);
//
//  // ----------------------end of borsh

  const ix = new TransactionInstruction(
    {
      keys: [],
      programId: vesting_program_id,
      data,
    }
  )

  const tx = new Transaction().add(ix);
  let tx_hash = await sendAndConfirmTransaction(connection, tx, [payer])
  console.log(tx_hash)
}

// ----------------------------------------------------------------------------- run

createContract()
// changeDest()
// unlockContract()
// callEmpty()
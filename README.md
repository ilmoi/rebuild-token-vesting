This started as a simple "training-wheels" project to rebuild bonfida's token
vesting contract.

# De/serializing

One useful thing that came out along the way is benchmarking of how expensive
different instruction serialization methods are.

I compared 3:
1. manual de/serialization of bytes
2. [rust's bincode](https://github.com/bincode-org/bincode)
3. [near's borsh](https://borsh.io/)

The results are:
```
manual
Program SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM consumed 3665 of 200000 compute units

bincode
Program SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM consumed 4796 of 200000 compute units

borsh
Program SoLi39YzAM2zEXcecy77VGbxLB5yHryNckY9Jx7yBKM consumed 3854 of 200000 compute units
```

Thus manual is least expensive, but borsh is close.

In terms of quantity/readability of code - I thought borsh would win since
deserializing in rust is 1 line vs writing out everything yourself (see
`rs/src/instruction.rs`). But js ended up being a shitshow - see `js/play.js` and https://github.com/near/borsh-js/issues/21

For now manual de/serialization seems optimal, unless borsh-js gets better enum handling.

# Fuzzing

I tried fuzzing with 2 different fuzzers, and managed to get both to work in
the end.

## [honggfuzz](https://github.com/rust-fuzz/honggfuzz-rs)
- unfortunately doesn't currently work on mac - see [this](https://github.com/rust-fuzz/honggfuzz-rs/issues/56)
- so instead can be run out of a docker container
- steps:
 - 1 build and run the docker image (mount the volume to make your life easier)
```
docker build -t vesting .
docker run -it -v $(pwd):/app/ vesting bash  
```
 - 2 cd into hfuzz dir and run this command (BPF_OUT_DIR=".." is needed for the
     fuzzer to work in bpf mode. Otherwise tests will be run as native code.)
```
BPF_OUT_DIR="/app/target/deploy" HFUZZ_RUN_ARGS="-t 10 -n 1 -N 1000000 -Q" cargo hfuzz run vesting_fuzz
```
 - 3 customize flags passed to hfuzz as per [this](https://github.com/google/honggfuzz/blob/master/docs/USAGE.md)

## [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
- works on mac, no problem
- cd into fuzz dir and run this command:
```
BPF_OUT_DIR="/Users/ilmoi/Dropbox/crypto_bc/sol/token-vesting/rebuild-token-vesting/rs/target/deploy" cargo-fuzz run fuzz_target_1
```

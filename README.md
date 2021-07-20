This started as a simple "training-wheels" project to rebuild bonfida's token
vesting contract.

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

# spinny

[![Build Status](https://dev.azure.com/jtnunley01/Beetle/_apis/build/status/not-a-seagull.spinny?branchName=master)](https://dev.azure.com/jtnunley01/Beetle/_build/latest?definitionId=6&branchName=master)
[![crates.io](https://img.shields.io/crates/v/spinny)](https://crates.io/crates/spinny)
[![docs.rs](https://docs.rs/spinny/badge.svg)](https://docs.rs/spinny)

Provides an `RwLock`-like struct that is `no_std` compatible and based on spinlocks. Made this because I couldn't find an equivalent that already existed that wasn't unmaintained.

# Thanks

Thank you to mvdnes for creating the original `spin` crate, which this crate is inspired by.

## License

Dual licensed under the MIT License and Apache-2.0 License at the user's option.

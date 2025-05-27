# Contracts

At this point everything surrounding this is research and speculations.

The general contract structure should be as follows:

```rust
#[contract]
impl Contract {
    pub fn public_function(&self, v1: u64, v2: Address) {
        let v3 = self.caller_address;

        ...
    }
}
```

`Contract` is a structure that would contain everything that is necessary to access the storage, find the call info, etc. It would most likely be defined in the syslib, and extended by the user. There are no details on how the storage interface would look like at this point.

The macro would do the following:
    - Generate the system code required to establish the allocator, the entry point, call the selected function
    - Iterate over the pub functions and compute the selectors.
    - perhaps rewrite the function's signature to receive the overlay over the call data (see the sibling doc)
        - if not, then the signature needs to be as follows `f(&self, v1: Enc<u64>, v2: Enc<Address>)`.
        - We may go with both approaches and alter the signature conditionally if it's not `Enc<T>`.
        - I haven't yet checked how rust analyzer behaves here, if it's going to be confused then rewriting the signature is not advisable.

Contract inheritance is supported by having traits. Traits don't need any special decoration and are just regular traits. Implementing several traits should look as follows:

```rust
#[contract]
mod contract {
    impl X for Contract { ... }

    impl Y for Contract { ... }
}
```

This is because Rust's macros don't support state (there are workarounds, but it's a pain and not very assuring) and we need to generate a single entry point with all the functions collected.

A possible issue may arise in the following case:

```rust
trait A { }
trait B: A { }

impl A for Contract {} 

#[contract]
impl B for Contract {}
```

In this case the functions from `A` wouldn't be handled and wouldn't be callable. It's not strictly and error, the contract would still compile and work, but can be a mistake. Macros can't check for this kind of things - they operate on syntax, not on semantics. Perhaps a warning can be emitted from a build script. Or a custom linter can be written that delegates to 'check', 'clippy' whatever, and adds additional lints. Or maybe [this](https://github.com/trailofbits/dylint) can do the job.

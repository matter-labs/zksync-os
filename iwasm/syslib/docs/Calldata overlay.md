# Calldata Overlay

This is the mechanism that allows accessing the calldata in a convenient and idiomatic manner. The overlay acts as a composite smart pointer to the calldata, and decodes it on demand.

Tests can be found in `./src/tests/abi/mod.rs`.

*Note: the word 'pointer' used in this doc refers to a smart pointer not to raw pointers.*

## Approach

The overlay uses a destructive read during the decoding disallowing direct access to the calldata. 

Overall the usage should look as follows:

```rust
let value: Enc<SomeType> = // Contract macro would provide this value. 
                           // It's also possible to create one by hand if needed - see tests.

let address = 
    value // This is a pointer to a struct and is dereferenced to a reflection type containing the fields.
    .user // This is `Enc<T>` - a nested pointer.
    .address; // This is `Enc<U160>`.

operate_on_address(address) // Address is pointer to a value and is dereferenced directly to `&U160`.
```

The dichotomy between the dereference targets allows for a clean lazy usage of the overlay. It also prevents decoding of the struct by mistake. In case the dereferencing is actually needed the user can `value.as_deref()` - derefing this will force decoding of the entire value.

### Values

Once the value is decoded, the slot that contains the data is rewritten with the native data, so any subsequent access doesn't incur decoding costs. For example for `u32` the big endian `U256` slot is rewritten with native endian `u32` at the slot address (leftmost bytes). In case of a string the length slot is rewritten with the `&str`.

The state of each slot is tracked with a state key - a single bit value. The state key set prepends the calldata and stored in a reverse order, so finding the slot and the slot state key requires only 2 values: the base pointer and slot index.

In some, when the expected calldata size allows, it is possible to reduce the size of the pointer to a single u32. With additional assumptions (whether call data can, perhaps conditionally, be put into a static location) it can be reduced to a single byte, when the expected calldata is 256 values or fewer.

### Unsized 

Byte arrays aren't decoded except for the length slot. When decoding the length slot is rewritten with the reference to the slice that points directly to the calldata bytes. 

### Composites

Composites are structs and tuples. They allow access to their member as an overlay members or allow decoding them on demand into their native representation:

```rust
#[derive(Encodable)]
struct A { v: u32 }

let overlay: Enc<A> = // ...;

// Get the pointer to `v`. It's not clear whether the `&` is necessary here. It may not be required for some cases.
let v: Enc<u32> = &overlay.v;

// Convert to native. This is costly for large structs, but convenient is the user is sure to require it entirety.
let a = &*overlay.as_deref();

```

Composites native representations aren't written into the calldata because they don't have the bytes that represent them. E.g. A struct over slot `A` has only bytes representing `A`. As an optimization it is possible to sometimes write them into the offset field of the composite, but this isn't a general solution. So generally the nested pointers and the native representations are written into the `Enc` on demand. The place for this data is still reserved, though it is relatively small compared to a single calldata slot.

### On state storage

Storing the state key outside the pointer simplifies the management of the pointers. When the state key is stored inside the pointer (e.g. tagging the raw pointer pointing to the calldata slot), copying the pointer may lead to double decoding. Storing the key in a single location syncs the pointers.

Pointers that don't have an external location to store the state key, must do it within them, which makes them non-cloneable. This leads to issues with partial borrows, vain decodes, and some issues with ergonomics, due to `Deref` returning `&T`. So the problem reduces to: where to store the state key for all types that may inhabit the calldata.

## Discarded alternatives

### Instantiating over calldata bytes

Initially the following was attempted:

```rust
struct ValueOverlay<T> {
    data: UnsafeCell<AbiSlot>,
    phantom: PhantomData<T>
}
```

This struct would be instantiated directly over the `AbiSlot` but this led to 2 issues:
    - The provided storage is limited to size of the slot, which works ok for types smaller than that, but a `U256` would have issues: there's no place where the state key could be stored.
    - There was an additional issue with nested dereferences, but I am absolutely unable to recall it.

Overall this solution can be elegant. It would use less memory and be more performant, but it's somewhat more complicated and would require special cases treatment when composing different types.

### Using raw pointers to reference slots

This is very similar to current solution, but the state key was stored as a raw pointer tag. This led to issues described in the state storage notes above, which required storing the decoded value inside the pointer. Also, raw pointers everywhere aren't fun.

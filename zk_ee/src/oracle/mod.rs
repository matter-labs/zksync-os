// we need some form of oracle to abstract away IO access to the system

// Oracle trait is abstract and only concrete queries will implement something for themselves to describe
// what oracle types they support. Even though we would really want to have type level checks if oracle supports
// certain query or not, and define queries are just blind key + value type, we also want to avoid excessive monomorphization
// and keep code size minimal for proving environments

pub mod usize_rw;

pub use self::usize_rw::*;

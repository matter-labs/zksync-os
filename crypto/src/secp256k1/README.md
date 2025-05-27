# secp256k1
A highly optimised implementation of ecrecover precompile with precomputed generator multiplication table (i.e. static context). It can run in two modes - native 64-bit and 32-bit with delegation calls for u256 arithmatic. 

The basic structure is based on the implementation found in [k256](https://github.com/RustCrypto/elliptic-curves/tree/master/k256), with optimisations and static context added from [libsecp256k1](https://github.com/bitcoin-core/secp256k1). Both are acknowledged in the source code where applicable.

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

contract arith {
  function fibonacciish(uint256 a, uint256 b, uint32 rounds) public pure returns (uint256) {
    uint256 a = a;
    uint256 b = b;
    uint32 cnt = 0;

    while (cnt < rounds) {
      cnt += 1;

      uint256 c = a + b;

      a = b;
      b = c;
    }

    return b;
  }
}

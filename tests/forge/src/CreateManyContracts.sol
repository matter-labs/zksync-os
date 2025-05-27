// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract ExampleContract {
    uint256 public number;
}

// Contract that creates many contracts - some using CREATE and some using CREATE2.
contract CreateTester {
    address public deployedAddress1;
    address public deployedAddress2;
    address public deployedAddress3;

    function run() public {
        // Deploy the first contract using create
        ExampleContract counter1 = new ExampleContract();
        deployedAddress1 = address(counter1);

        // Deploy the second contract using create2
        bytes32 salt = keccak256(abi.encodePacked("salt"));
        ExampleContract counter2 = new ExampleContract{salt: salt}();
        deployedAddress2 = address(counter2);

        // Deploy the third contract using create
        ExampleContract counter3 = new ExampleContract();
        deployedAddress3 = address(counter3);
    }
}

// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {ImplementationContract, DelegateCalls, MainContract} from "../src/DelegateCalls.sol";

contract CreateTesterScript is Script {
    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        MainContract mainContract = new MainContract();
        // Should return 18 (15 in storage + 3 that ImplementationContract adds)
        console.log("Result: ", mainContract.run());
        vm.stopBroadcast();
    }
}

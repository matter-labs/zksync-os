// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {CreateTester} from "../src/CreateManyContracts.sol";

contract CreateTesterScript is Script {
    CreateTester public createTester;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        createTester = new CreateTester();
        createTester.run();

        vm.stopBroadcast();
    }
}

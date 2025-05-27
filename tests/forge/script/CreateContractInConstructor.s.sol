// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {CreateInConstructorTester} from "../src/CreateContractInConstructor.sol";

contract CreateInConstructorTesterScript is Script {
    CreateInConstructorTester public createTester;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        createTester = new CreateInConstructorTester();
        createTester.run();

        vm.stopBroadcast();
        console.log(
            "First in constructor: ",
            createTester.deployedAddressConstructor1()
        );
        console.log(
            "Second in constructor: ",
            createTester.deployedAddressConstructor2()
        );

        console.log("First in run: ", createTester.deployedAddress1());
        console.log("Second in run: ", createTester.deployedAddress2());
        console.log("Third in run: ", createTester.deployedAddress3());
    }
}

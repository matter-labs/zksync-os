// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract ImplementationContract {
    uint256 public number;

    function getNumber() public view returns (uint256) {
        return number + 3;
    }
    function incNumber() public {
        number += 1;
    }
}

contract DelegateCalls {
    uint256 public number;
    address public implementation;

    constructor(address _implementation) {
        implementation = _implementation;
        number = 15;
    }

    function incrementViaDelegate() public {
        (bool success, ) = implementation.delegatecall(
            abi.encodeWithSignature("incNumber()")
        );
        require(success, "Delegate call failed");
        number += 1;
    }

    function getViaDelegate() public returns (uint256) {
        (bool success, bytes memory data) = implementation.delegatecall(
            abi.encodeWithSignature("getNumber()")
        );
        require(success, "Delegate call failed");
        return abi.decode(data, (uint256));
    }
}

contract MainContract {
    function run() public returns (uint256) {
        ImplementationContract implementation = new ImplementationContract();
        DelegateCalls delegateCalls = new DelegateCalls(
            address(implementation)
        );
        address target = address(delegateCalls);
        // do a static call to get the number
        (bool success, bytes memory data) = target.staticcall(
            abi.encodeWithSignature("getViaDelegate()")
        );
        require(success, "Delegate call failed");
        uint256 result = abi.decode(data, (uint256));

        require(result == 18, "Result should be 18");
        return result;
    }
}

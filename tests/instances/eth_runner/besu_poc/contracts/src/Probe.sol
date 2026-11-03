// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

/// Records block-context values so they end up in the state root and receipts.
contract Probe {
    uint256 public chainId;
    address public coinbase;
    uint256 public prevrandao;
    bytes32 public parentHash;
    uint256 public baseFee;
    uint256 public timestamp;
    uint256 public number;
    uint256 public gasLimit;
    uint256 public calls;

    event Recorded(uint256 indexed number, bytes32 parentHash, uint256 prevrandao);

    function record() external {
        chainId = block.chainid;
        coinbase = block.coinbase;
        prevrandao = block.prevrandao;
        parentHash = blockhash(block.number - 1);
        baseFee = block.basefee;
        timestamp = block.timestamp;
        number = block.number;
        gasLimit = block.gaslimit;
        calls += 1;
        emit Recorded(block.number, parentHash, prevrandao);
    }
}

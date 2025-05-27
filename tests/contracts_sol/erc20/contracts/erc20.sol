// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

contract erc20 {
  uint256 _totalSupply;
  mapping(address acc => uint256 balance) balances;
  mapping(address acc => mapping(address spender => uint amount)) allowances;

  event Transfer(address indexed _from, address indexed _to, uint256 _value);
  event Approval(address indexed _owner, address indexed _spender, uint256 _value);

  function name() public pure returns (string memory) {
    return "Test Token";
  }

  function symbol() public pure returns (string memory) {
    return "TST";
  }

  function decimals() public pure returns (uint8) {
    return 0;
  }

  function totalSupply() public view returns (uint256) {
    return _totalSupply;
  }

  function balanceOf(address _owner) public view returns (uint256 balance) {
    return balances[_owner];
  }

  function transfer(address _to, uint256 _value) public returns (bool success) {
    address _from = msg.sender;

    require(_from != address(0), "Sender is 0");
    require(_to != address(0), "Receiver is 0");

    require(balances[_from] > _value, "Not enough funds.");

    balances[_from] -= _value;
    balances[_to] += _value;

    emit Transfer(_from, _to, _value);

    return true;
  }

  function approve(address _spender, uint256 _value) public returns (bool success) {
    address _from = msg.sender;

    require(_from != address(0), "Approver is 0");
    require(_spender != address(0), "Spender is 0");

    allowances[_from][_spender] = _value;

    emit Approval(_from, _spender, _value);

    return true;
  }

  function transferFrom(address _from, address _to, uint256 _value) public returns (bool success) {
    address _spender = msg.sender;

    require(_spender != address(0), "Spender is 0");
    require(_from != address(0), "Sender is 0");
    require(_to != address(0), "Receiver is 0");

    uint256 balance = balances[_from];
    uint256 _allowance = allowances[_from][_spender];

    require(_value <= balance, "Balance is not high enough");
    require(_value <= _allowance, "Allowance is smaller than transfer");

    allowances[_from][_spender] -= _value;
    balances[_from] -= _value;

    balances[_to] += _value;
    emit Transfer(_from, _to, _value);

    return true;
  }

  function allowance(address _owner, address _spender) public view returns (uint256 remaining) {
    require(_owner != address(0), "Owner is 0");
    require(_spender != address(0), "Spender is 0");

    return allowances[_owner][_spender];
  }

  // Custom

  function mint(address addr, uint256 _amount) public returns (bool success) {
    _totalSupply = _totalSupply + _amount;
    balances[addr] += _amount;

    return true;
  }
}

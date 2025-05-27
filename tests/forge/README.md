Test contracts, used by tests in instance_examples.



To test things:


Start anvil & use wallet 3 from there:

Address: 0x90F79bf6EB2c4f870365E785982E1f101E93b906
Private key: 0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6



```
forge script script/CreateManyContracts.s.sol --rpc-url http://localhost:8044 --private-key 0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
```

Then collect the bytecode & results from out & broadcast directories.
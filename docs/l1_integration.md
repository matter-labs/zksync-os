# L1 integration

ZKsyncOS will be used for zk rollup/validium, so we need to support 2 things:

- proving state transition on the settlement layer with ZK proofs.

- sending messages from and to the settlement layer(l1)

## Proving

Currently, proving pipeline and public inputs are under development, so this part is not finalized, and is out of scope.

## Messaging

Users can send l1 -> l2 transaction, see [L1 -> L2 transactions](./transaction_processing.md)

Also, there is a way to send messages from l2 -> l1, for this users have to make a call to a special system address.
It's implemented via system hook, see [L1 messenger system hook](../system_hooks.md)

Please note, that now ZKsyncOS just collects messages and returns them.
In the future, we'll have to include their commitment to the public input to open on l1.

#![feature(generic_const_exprs)]
#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

use syslib::{contract, storage::kinds::{Mapping, Stored, Value}, types::{ints::{U256, U256BE}, Address}};

struct Contract<'a> {
    total_supply: Stored<Value<U256>>,
    balances: Stored<Mapping<'a, Address, U256>>,
    allowances: Stored<Mapping<'a, Address, Mapping<'a, Address, U256>>>,
}

impl Default for Contract<'_> {
    fn default() -> Self {
        Self { 
            total_supply: Stored::new(0, Value::default()),
            balances: Stored::new(1, Mapping::instantiate(&U256BE::from_usize(1))),
            allowances: Stored::new(2, Mapping::new(U256BE::from_usize(2))) }
    }
}

#[contract]
impl<'a> Contract<'a> {
    pub fn name(&self) -> Result<&str, &str> {
        Ok(&"Test token")
    }

    pub fn symbol(&self) -> Result<&str, &str> {
        Ok(&"TST")
    }

    pub fn total_supply(&self) -> Result<U256, &str> {
        Ok(self.total_supply.read())
    }

    pub fn balanceOf(&self, owner: &Address) -> Result<U256, &str> {
        let balance = self.balances.read(&*owner);
        Ok(balance)
    }

    pub fn transfer(&mut self, to: &Address, amount: &U256) -> Result<bool, &str> {

        let from = syslib::system::msg::sender();

        if from.is_zero() {
            return Err("from is zero");
        }

        if to.is_zero() {
            return Err("to is zero");
        }

        let mut bal_from = self.balances.entry(&from);

        if bal_from.value() < *amount {
            return Err("Not enough funds");
        }

        let mut bal_to = self.balances.entry(&to);

        bal_from.write_value(bal_from.value().sub(*amount));
        bal_to.write_value(bal_to.value().add(*amount));

        Ok(true)
    }

    pub fn allowance(&mut self, address: &Address) -> Result<U256, &str> {
        let sender = Address::from_usize(0);

        let allow_from = self.allowances.get(&sender);

        let allowed = allow_from.read(&address);

        Ok(allowed)
    }

    pub fn mint_for(&mut self, address: &Address, amount: &U256) -> Result<U256, &str> {
        let ts = self.total_supply.read();
        let r = ts.add(*amount);
        self.total_supply.write(&r);

        let mut addr_b = self.balances.entry(&address);

        addr_b.write_value(addr_b.value().add(*amount));

        Ok(r)
    }
}

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::wee_alloc;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Balance, Promise, PromiseResult, Gas};
use near_sdk::json_types::U128;
use std::collections::HashMap;

pub type WrappedBalance = U128;

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

pub const CALLBACK: Gas = 3_000_000_000_000; // 3 TGas

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(Default, BorshDeserialize, BorshSerialize)]
pub struct Multisender {
    deposits: HashMap<String, Balance>,
}

#[ext_contract(ext_self)]
pub trait ExtMultisender {
    fn on_transfer_from_balance(&mut self, account_id: AccountId, amount_sent: U128, recipient: AccountId);
    fn on_transfer_attached_tokens(&mut self, account_id: AccountId, amount_sent: U128, recipient: AccountId);
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Operation {
    account_id: AccountId,
    amount: WrappedBalance,
}


#[near_bindgen]
impl Multisender {
    #[payable]
    pub fn multisend_attached_tokens(&mut self, accounts: Vec<Operation>) {
        let account_id = env::predecessor_account_id();
        let tokens: u128 = near_sdk::env::attached_deposit();

        let mut total: Balance = 0;
        for account in &accounts {
            assert!(
                env::is_valid_account_id(account.account_id.as_bytes()),
                "Account @{} is invalid",
                account.account_id
            );

            let amount: Balance = account.amount.into();
            total += amount;
        }

        assert!(
            total <= tokens,
            "Not enough attached tokens to run multisender (Supplied: {}. Demand: {})",
            tokens,
            total
        );

        let direct_logs: bool = accounts.len() < 100;
        let mut logs: String = "".to_string();

        for account in accounts {
            let amount_u128: u128 = account.amount.into();

            if direct_logs {
                env::log(format!("Sending {} yNEAR (~{} NEAR) to account @{}", amount_u128, yton(amount_u128), account.account_id).as_bytes());
            } else {
                let log = format!("Sending {} yNEAR (~{} NEAR) to account @{}\n", amount_u128, yton(amount_u128), account.account_id);
                logs.push_str(&log);
            }

            Promise::new(account.account_id.clone())
                .transfer(amount_u128)
                .then(
                    ext_self::on_transfer_attached_tokens(
                        account_id.clone(),
                        account.amount,
                        account.account_id,
                        &env::current_account_id(),
                        0,
                        CALLBACK,
                    )
                );
        }

        if !direct_logs {
            env::log(format!("Done!\n{}", logs).as_bytes());
        }
    }

    pub fn multisend_from_balance(&mut self, accounts: Vec<Operation>) {
        let account_id = env::predecessor_account_id();

        assert!(self.deposits.contains_key(&account_id), "Unknown user");

        let mut tokens: Balance = *self.deposits.get(&account_id).unwrap();
        let mut total: Balance = 0;
        for account in &accounts {
            assert!(
                env::is_valid_account_id(account.account_id.as_bytes()),
                "Account @{} is invalid",
                account.account_id
            );

            let amount: Balance = account.amount.into();
            total += amount;
        }

        assert!(
            total <= tokens,
            "Not enough deposited tokens to run multisender (Supplied: {}. Demand: {})",
            tokens,
            total
        );

        let mut logs: String = "".to_string();
        let direct_logs: bool = accounts.len() < 100;

        for account in accounts {
            let amount_u128: u128 = account.amount.into();

            if direct_logs {
                env::log(format!("Sending {} yNEAR (~{} NEAR) to account @{}", amount_u128, yton(amount_u128), account.account_id).as_bytes());
            } else {
                let log = format!("Sending {} yNEAR (~{} NEAR) to account @{}\n", amount_u128, yton(amount_u128), account.account_id);
                logs.push_str(&log);
            }

            tokens -= amount_u128;
            self.deposits.insert(account_id.clone(), tokens);

            Promise::new(account.account_id.clone())
                .transfer(amount_u128)
                .then(
                    ext_self::on_transfer_from_balance(
                        account_id.clone(),
                        account.amount,
                        account.account_id,
                        &env::current_account_id(),
                        0,
                        CALLBACK,
                    )
                );
        }

        if !direct_logs {
            env::log(format!("Done!\n{}", logs).as_bytes());
        }
    }

    pub fn multisend_from_balance_unsafe(&mut self, accounts: Vec<Operation>) {
        let account_id = env::predecessor_account_id();

        assert!(self.deposits.contains_key(&account_id), "Unknown user");

        let tokens: Balance = *self.deposits.get(&account_id).unwrap();
        let mut total: Balance = 0;
        for account in &accounts {
            assert!(
                env::is_valid_account_id(account.account_id.as_bytes()),
                "Account @{} is invalid",
                account.account_id
            );

            let amount: Balance = account.amount.into();
            total += amount;
        }

        assert!(
            total <= tokens,
            "Not enough deposited tokens to run multisender (Supplied: {}. Demand: {})",
            tokens,
            total
        );

        let mut logs: String = "".to_string();
        let direct_logs: bool = accounts.len() < 100;

        for account in accounts {
            let amount_u128: u128 = account.amount.into();

            Promise::new(account.account_id.clone()).transfer(account.amount.into());

            if direct_logs {
                env::log(format!("Sending {} yNEAR to account @{}", amount_u128, account.account_id).as_bytes());
            } else {
                let log = format!("Sending {} yNEAR to account @{}\n", amount_u128, account.account_id);
                logs.push_str(&log);
            }
        }

        self.deposits.insert(account_id, tokens - total);

        if !direct_logs {
            env::log(format!("Done!\n{}", logs).as_bytes());
        }
    }

    pub fn on_transfer_from_balance(&mut self, account_id: AccountId, amount_sent: U128, recipient: AccountId) {
        assert_self();

        let transfer_succeeded = is_promise_success();
        if !transfer_succeeded {
            env::log(format!("Transaction to @{} failed. {} yNEAR (~{} NEAR) kept on the app deposit", recipient, amount_sent.0, yton(amount_sent.0)).as_bytes());
            let previous_balance: u128 = self.get_deposit(account_id.clone()).into();
            self.deposits.insert(account_id, previous_balance + amount_sent.0);
        }
    }

    pub fn on_transfer_attached_tokens(&mut self, account_id: AccountId, amount_sent: U128, recipient: AccountId) {
        assert_self();

        let transfer_succeeded = is_promise_success();
        if !transfer_succeeded {
            env::log(format!("Transaction to @{} failed. {} yNEAR (~{} NEAR) moved to the app deposit", recipient, amount_sent.0, yton(amount_sent.0)).as_bytes());
            let previous_balance: u128 = self.get_deposit(account_id.clone()).into();
            self.deposits.insert(account_id, previous_balance + amount_sent.0);
        }
    }

    #[payable]
    pub fn deposit(&mut self) {
        let attached_tokens: Balance = near_sdk::env::attached_deposit();
        let account_id = env::predecessor_account_id();

        let previous_balance: u128 = self.get_deposit(account_id.clone()).into();
        self.deposits.insert(account_id, previous_balance + attached_tokens);
    }

    pub fn withdraw(&mut self) -> Promise {
        let account_id = env::predecessor_account_id();

        assert!(self.deposits.contains_key(&account_id), "Unknown user");

        let tokens: Balance = *self.deposits.get(&account_id).unwrap();
        assert!(tokens > 0, "Nothing to withdraw");

        env::log(
            format!(
                "@{} withdrawing {}",
                account_id, tokens
            )
                .as_bytes(),
        );

        self.deposits.insert(account_id.clone(), 0);
        Promise::new(account_id).transfer(tokens)
    }

    pub fn get_deposit(&self, account_id: String) -> U128 {
        match self.deposits.get(&account_id) {
            Some(deposit) => {
                U128::from(*deposit)
            }
            None => {
                0.into()
            }
        }
    }
}

pub fn assert_self() {
    assert_eq!(env::predecessor_account_id(), env::current_account_id());
}

fn is_promise_success() -> bool {
    assert_eq!(
        env::promise_results_count(),
        1,
        "Contract expected a result on the callback"
    );
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

pub fn yton(yocto_amount: Balance) -> Balance {
    (yocto_amount + (5 * 10u128.pow(23))) / 10u128.pow(24)
}


#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    fn master_account() -> AccountId { "admin.near".to_string() }

    fn alice_account() -> AccountId { "alice.near".to_string() }

    fn bob_account() -> AccountId { "bob.near".to_string() }

    pub fn get_context(
        predecessor_account_id: AccountId,
        attached_deposit: u128,
        is_view: bool,
    ) -> VMContext {
        VMContext {
            current_account_id: predecessor_account_id.clone(),
            signer_account_id: predecessor_account_id.clone(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 1,
            block_timestamp: 0,
            epoch_height: 1,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 10u64.pow(6),
            attached_deposit,
            prepaid_gas: 10u64.pow(15),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
        }
    }

    fn ntoy(near_amount: Balance) -> Balance {
        near_amount * 10u128.pow(24)
    }

    #[test]
    fn test_deposit() {
        let context = get_context(alice_account(), ntoy(100), false);
        testing_env!(context.clone());

        let mut contract = Multisender::default();

        contract.deposit();

        assert_eq!(
            ntoy(100),
            contract.get_deposit(alice_account()).0
        );
    }

    #[test]
    fn test_deposit_withdraw() {
        let context = get_context(alice_account(), ntoy(100), false);
        testing_env!(context.clone());

        let mut contract = Multisender::default();

        contract.deposit();
        contract.withdraw();

        assert_eq!(
            0,
            contract.get_deposit(alice_account()).0
        );
    }
}

use std::{collections::HashSet, str::FromStr};

use borsh::BorshSerialize;
use ft_external::fungible_token_trait;
use mpc_external::mpc_trait;
use near_sdk::{
    collections::{LookupMap, UnorderedMap},
    env::{self, predecessor_account_id},
    json_types::{U128, U64},
    near, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise,
};

pub mod ft_external;
pub mod mpc_external;
pub mod runes;
pub mod types;

use runes::*;
use types::*;

// near ca ref: https://github.com/near-examples/near-multichain

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    runes: UnorderedMap<Ticker, Rune>,
    mpc_contract: AccountId,
    usdt_contract: AccountId,
    admin: AccountId,
    user_runes: LookupMap<AccountId, HashSet<Ticker>>,
}

#[near(serializers=[borsh])]
#[derive(PanicOnDefault)]
pub struct OldContract {
    runes: UnorderedMap<Ticker, Rune>,
    mpc_contract: AccountId,
    usdt_contract: AccountId,
    admin: AccountId,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub enum ContractStorageKeys {
    Runes,
    RunesBalance { name: String },
    UserRunes,
}

#[near]
impl Contract {
    #[init]
    #[private]
    pub fn new(mpc_contract: AccountId, admin: AccountId, usdt_contract: AccountId) -> Self {
        Contract {
            runes: UnorderedMap::new(ContractStorageKeys::Runes),
            mpc_contract,
            usdt_contract,
            admin,
            user_runes: LookupMap::new(ContractStorageKeys::UserRunes),
        }
    }

    #[init]
    #[private]
    pub fn new_default(admin: AccountId) -> Self {
        Self::new(
            AccountId::from_str("signer.canhazgas.testnet").unwrap(),
            admin,
            AccountId::from_str("wrap.testnet").unwrap(),
        )
    }

    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let old_state: OldContract = env::state_read().expect("failed");

        // return the new state
        let new_self = Self {
            runes: old_state.runes,
            mpc_contract: old_state.mpc_contract,
            usdt_contract: old_state.usdt_contract,
            admin: old_state.admin,
            user_runes: LookupMap::new(ContractStorageKeys::UserRunes),
        };

        return new_self;
    }

    pub fn change_mpc_contract(&mut self, mpc_contract: AccountId) {
        self.assert_admin();
        self.mpc_contract = mpc_contract;
    }

    // TODO: storage_deposit
    pub fn new_rune(
        &mut self,
        ticker: String,
        launch_type: String,
        total: U128,
        price: U128,
        creator_address: AccountId,
    ) {
        self.assert_admin();

        assert!(
            self.runes.get(&ticker).is_none(),
            "Runes with the same name already exist {}",
            ticker
        );

        if launch_type == "FixedPrice" {
            // Todo: implement from_str for launchtype
            self.runes.insert(
                &ticker.clone(),
                &Rune::new(
                    ticker,
                    LaunchType::FixedPrice,
                    total.0,
                    price.0,
                    creator_address,
                ),
            );
        }
    }

    pub fn get_rune(&self, ticker: String) -> RuneOutput {
        let rune = self.runes.get(&ticker).expect("Rune doesn't exist");

        RuneOutput {
            ticker,
            total: U128::from(rune.total),
            minted: U128::from(rune.minted),
            price: U128::from(rune.price),
        }
    }

    pub fn get_runes(&self, from_index: U64, limit: Option<U64>) -> Vec<RuneOutput> {
        self.runes
            .iter()
            .skip(from_index.0 as usize)
            .take(limit.unwrap_or(U64::from(10)).0 as usize)
            .map(|(ticker, rune)| RuneOutput {
                ticker,
                total: U128::from(rune.total),
                minted: U128::from(rune.minted),
                price: U128::from(rune.price),
            })
            .collect()
    }

    pub fn get_rune_balances(
        &self,
        from_index: U64,
        limit: Option<U64>,
        account_id: AccountId,
    ) -> Vec<RuneBalance> {
        self.user_runes
            .get(&account_id)
            .unwrap_or(HashSet::new())
            .iter()
            .skip(from_index.0 as usize)
            .take(limit.unwrap_or(U64::from(10)).0 as usize)
            .map(|ticker| {
                let balance = self.get_rune_balance(ticker.clone(), account_id.clone());
                return RuneBalance {
                    ticker: ticker.clone(),
                    balance,
                };
            })
            .collect()
    }

    pub fn get_rune_balance(&self, ticker: String, account_id: AccountId) -> U128 {
        let rune = self.runes.get(&ticker).expect("Rune doesn't exist");

        U128::from(rune.get_balance(account_id))
    }

    pub fn withdraw(
        &mut self,
        ticker: String,
        account_id: AccountId,
        bitcoin_address: BitcoinAddress,
    ) {
        self.assert_admin();

        let mut rune = self.runes.get(&ticker).expect("Rune doesn't exist");

        let amount = rune.withdraw(account_id.clone());

        if amount > 0 {
            env::log_str(format!("{} withdraw to {}", account_id, bitcoin_address).as_str());
            self.runes.insert(&ticker, &rune);

            let user_runes = self.user_runes.get(&account_id);

            if let Some(mut user_runes) = user_runes {
                user_runes.remove(&ticker);

                self.user_runes.insert(&account_id, &user_runes);
            }
        }
    }

    pub fn creator_withdraw(&mut self, ticker: String) -> U128 {
        let mut rune = self.runes.get(&ticker).expect("Rune doesn't exist");

        assert_eq!(rune.creator_address, env::predecessor_account_id());

        let amount = U128::from(rune.creator_withdraw());

        if amount.0 > 0 {
            self.runes.insert(&ticker, &rune);

            fungible_token_trait::ext(self.usdt_contract.clone())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(Gas::from_tgas(100))
                .ft_transfer(rune.creator_address.clone(), amount, None)
                .then(Self::ext(env::current_account_id()).on_creator_withdraw(ticker, amount));
        }

        amount
    }

    #[private]
    pub fn on_creator_withdraw(
        &mut self,
        #[callback_result] call_result: Result<(), near_sdk::PromiseError>,
        ticker: String,
        amount: U128,
    ) {
        if call_result.is_err() {
            let mut rune = self.runes.get(&ticker).expect("Rune doesn't exist");
            rune.creator_withdraw_failed(amount.0);
            self.runes.insert(&ticker, &rune);
        };
    }

    pub fn sign(&mut self, payload: Vec<u8>, ticker: String, key_version: u32) -> Promise {
        // sign as derived address
        self.assert_admin();

        mpc_trait::ext(self.mpc_contract.clone())
            .with_static_gas(Gas::from_tgas(30))
            .sign(payload.as_slice().try_into().unwrap(), ticker)
    }
}

// utils
#[near]
impl Contract {
    fn assert_admin(&self) {
        assert_eq!(self.admin, predecessor_account_id(), "Not owner");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{test_utils::VMContextBuilder, testing_env, NearToken};

    fn set_context(predecessor: &str) {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor.parse().unwrap());
        builder.attached_deposit(NearToken::from_near(1));

        testing_env!(builder.build());
    }

    #[test]
    fn mint_and_get_success() {
        let mut contract = Contract::new_default(AccountId::from_str("admin").unwrap());
        let ticker = "example".to_string();

        set_context("admin");
        contract.new_rune(
            ticker.clone(),
            "FixedPrice".to_string(),
            U128::from(1000),
            U128::from(1),
            AccountId::from_str("creator").unwrap(),
        );
    }

    #[test]
    fn always_true() {
        assert_eq!(true, true);
    }
}

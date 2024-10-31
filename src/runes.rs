use near_sdk::{json_types::U128, near, store::LookupMap, AccountId};

use crate::ContractStorageKeys;

#[near(serializers=[borsh])]
pub enum LaunchType {
    FixedPrice,
    BondingCurve, // v2
}

#[near(serializers=[borsh])]
pub struct Rune {
    pub ticker: String,
    pub launch_type: LaunchType,
    pub total: u128,
    pub minted: u128,
    pub price: u128, // in usdt
    pub balance: LookupMap<AccountId, u128>,
    pub creator_balance: u128,
    pub creator_address: AccountId,
}

#[near(serializers=[json])]
pub struct RuneOutput {
    pub ticker: String,
    pub total: U128,
    pub minted: U128,
    pub price: U128,
}

#[near(serializers=[json])]
pub struct RuneBalance {
    pub ticker: String,
    pub balance: U128,
}

impl Rune {
    pub fn new(
        name: String,
        launch_type: LaunchType,
        total: u128,
        price: u128,
        creator_address: AccountId,
    ) -> Self {
        Self {
            ticker: name.clone(),
            launch_type,
            total,
            minted: 0,
            price,
            balance: LookupMap::new(ContractStorageKeys::RunesBalance { name }),
            creator_balance: 0,
            creator_address,
        }
    }

    // {derivation_path} will be used for frontend to derive the runes launchpad address to
    // bitcoin address (for premine holder) and ethereum address (for payment)
    pub fn get_derivation_path(&self) -> String {
        self.ticker.clone()
    }

    pub fn mint(&mut self, usdt_value: u128, account_id: AccountId) {
        // usdt value to amount
        let amount = usdt_value / self.price;
        assert!(amount > 0, "Your transferred amount is insufficient");
        assert!(amount + self.minted <= self.total, "Insufficient supply");

        let previous_balance = self.balance.get(&account_id).unwrap_or(&0);
        self.balance.insert(account_id, previous_balance + amount);

        self.minted += amount;
        self.creator_balance += usdt_value;
    }

    pub fn get_balance(&self, account_id: AccountId) -> u128 {
        *self.balance.get(&account_id).unwrap_or(&0)
    }

    pub fn withdraw(&mut self, account_id: AccountId) -> u128 {
        let previous_balance = *self.balance.get(&account_id).unwrap_or(&0);
        self.balance.insert(account_id, 0);

        previous_balance
    }

    pub fn creator_withdraw(&mut self) -> u128 {
        let creator_balance = self.creator_balance;
        self.creator_balance = 0;

        creator_balance
    }

    pub fn creator_withdraw_failed(&mut self, amount: u128) {
        self.creator_balance += amount;
    }
}

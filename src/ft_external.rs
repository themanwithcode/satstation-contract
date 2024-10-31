use std::collections::HashMap;

use super::*;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    env::{self},
    ext_contract, PromiseOrValue,
};

#[ext_contract(fungible_token_trait)]
#[allow(dead_code)]
trait FungibleTokenContract {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[near]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String, // rune_name
    ) -> PromiseOrValue<U128> {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.usdt_contract,
            "Only supports usdt contract",
        );

        let ticker = msg.clone();
        let mut rune = self.runes.get(&ticker).expect("Rune doesn't exist");

        env::log_str(
            format!(
                "in {} tokens from @{} ft_on_transfer, msg = {}",
                amount.0, sender_id, msg,
            )
            .as_str(),
        );

        rune.mint(amount.0, sender_id.clone());
        self.runes.insert(&ticker, &rune);

        let user_runes = self.user_runes.get(&sender_id);

        if let Some(mut user_runes) = user_runes {
            user_runes.insert(ticker);

            self.user_runes.insert(&sender_id, &user_runes);
        } else {
            let mut new = HashSet::new();
            new.insert(ticker);
            self.user_runes.insert(&sender_id, &new);
        }

        PromiseOrValue::Value(U128::from(0))
    }
}

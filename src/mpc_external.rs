use near_sdk::{ext_contract, serde::Serialize, Promise};

#[ext_contract(mpc_trait)]
#[allow(dead_code)]
pub trait MPC {
    fn sign(&self, payload: Vec<u8>, path: String) -> Promise;
}

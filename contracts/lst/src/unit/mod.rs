use crate::*;
use near_sdk::test_utils::VMContextBuilder;
pub use near_sdk::testing_env;

mod unit_balance;
mod unit_event;
// mod unit_account;
// mod unit_api;
// mod unit_conversion;

pub fn alice_id() -> AccountId {
    "alice_id".parse().unwrap()
}

pub fn source_token_id() -> AccountId {
    "source_token_id".parse().unwrap()
}

pub fn target_token_id() -> AccountId {
    "target_token_id".parse().unwrap()
}

pub fn owner_id() -> AccountId {
    "owner_id".parse().unwrap()
}

pub fn user_id() -> AccountId {
    "user_id".parse().unwrap()
}

pub fn init_unit_env() -> UnitEnv {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(owner_id()).build());
    let contract = Contract::new(owner_id(), None);
    UnitEnv { contract, context }
}

pub fn init_contract() -> Contract {
    Contract::new(owner_id(), None)
}

pub struct UnitEnv {
    pub contract: Contract,
    pub context: VMContextBuilder,
}

impl UnitEnv {}

#[macro_export]
macro_rules! assert_err_string {
    ($expr:expr, $expected_substr:expr) => {{
        match $expr {
            Ok(_) => panic!("Expected error but got Ok"),
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains($expected_substr),
                    "Expected error message to contain '{}', but got '{}'",
                    $expected_substr,
                    msg
                );
            }
        }
    }};
}

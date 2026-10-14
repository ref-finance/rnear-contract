use crate::*;
use near_sdk::test_utils::VMContextBuilder;
pub use near_sdk::testing_env;

mod unit_event;
mod unit_platform;
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
    let contract = Contract::new(owner_id(), None, "wrap.near".parse().unwrap());
    UnitEnv { contract, context }
}

pub fn init_contract() -> Contract {
    Contract::new(owner_id(), None, "wrap.near".parse().unwrap())
}

#[test]
fn migrated_wnear_requires_one_time_owner_configuration() {
    let mut context = VMContextBuilder::new();
    context.predecessor_account_id(owner_id()).account_balance(NearToken::from_near(100));
    testing_env!(context.build());
    let initialized = Contract::new(owner_id(), None, "wrap.near".parse().unwrap());
    assert_eq!(initialized.get_emergency_storage_reserve(), U128(0));
    assert_eq!(near_sdk::borsh::to_vec(&initialized).unwrap()[0], 1);
    let VersionedContractData::Current(data) = initialized.data else {
        panic!("Expected current state");
    };
    let supply = data.token.total_supply;
    let old = ContractDataV1 {
        token: data.token,
        metadata: data.metadata,
        owner_id: data.owner_id,
        total_staked_near_amount: data.total_staked_near_amount,
        accounts: data.accounts,
        account_storage_usage: data.account_storage_usage,
        beneficiaries: data.beneficiaries,
        validator_pool: data.validator_pool,
        whitelist_account_id: data.whitelist_account_id,
        epoch_requested_stake_amount: data.epoch_requested_stake_amount,
        epoch_requested_unstake_amount: data.epoch_requested_unstake_amount,
        stake_amount_to_settle: data.stake_amount_to_settle,
        unstake_amount_to_settle: data.unstake_amount_to_settle,
        last_settlement_epoch: data.last_settlement_epoch,
    };
    let previous = Contract {
        data: VersionedContractData::V1(old),
    };
    assert_eq!(near_sdk::borsh::to_vec(&previous).unwrap()[0], 0);
    env::state_write(&previous);
    let mut migrated = Contract::migrate_state();
    assert_eq!(migrated.data().token.total_supply, supply);
    assert_eq!(migrated.get_wnear_id(), None);
    assert_eq!(migrated.get_emergency_storage_reserve(), U128(0));

    let unstake_message = UnstakeMessage {
        receiver_id: owner_id(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    context.predecessor_account_id(env::current_account_id());
    testing_env!(context.build());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        near_contract_standards::fungible_token::receiver::FungibleTokenReceiver::ft_on_transfer(
            &mut migrated,
            owner_id(),
            U128(1),
            near_sdk::serde_json::to_string(&unstake_message).unwrap(),
        )
    }))
    .is_err());

    let wnear_id: AccountId = "wrap.near".parse().unwrap();
    context.predecessor_account_id(user_id()).attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| migrated.set_wnear_id(wnear_id.clone()))).is_err());
    assert_eq!(migrated.get_wnear_id(), None);

    context.predecessor_account_id(owner_id()).attached_deposit(NearToken::from_yoctonear(0));
    testing_env!(context.build());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| migrated.set_wnear_id(wnear_id.clone()))).is_err());
    assert_eq!(migrated.get_wnear_id(), None);

    context.attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    migrated.set_wnear_id(wnear_id.clone());
    assert_eq!(migrated.get_wnear_id(), Some(wnear_id.clone()));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| migrated.set_wnear_id(wnear_id))).is_err());
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

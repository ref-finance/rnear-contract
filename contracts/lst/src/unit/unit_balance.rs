use crate::*;
use near_sdk::{test_utils, test_utils::VMContextBuilder, testing_env};

fn setup(share_assets: u128) -> (Contract, Validator) {
    let mut context = VMContextBuilder::new();
    testing_env!(context
        .current_account_id("lst.test".parse().unwrap())
        .predecessor_account_id(owner_id())
        .epoch_height(10)
        .account_balance(NearToken::from_near(100))
        .build());
    let mut contract = Contract::new(owner_id(), None);
    contract.mint_lst(&owner_id(), 40 * ONE_NEAR, Some("test"));
    contract.data_mut().total_staked_near_amount = share_assets;

    let mut validator = Validator::new("validator.test".parse().unwrap(), 10);
    validator.staked_amount = 30 * ONE_NEAR;
    validator.unstaked_amount = 20 * ONE_NEAR;
    validator.base_stake_amount = 10 * ONE_NEAR;
    validator.executing = true;
    contract.data_mut().validator_pool.total_weight = 10;
    contract.data_mut().validator_pool.total_base_stake_amount = 10 * ONE_NEAR;
    contract
        .data_mut()
        .validator_pool
        .save_validator(&validator);
    (contract, validator)
}

fn account(staked: u128, unstaked: u128) -> HumanReadableAccount {
    HumanReadableAccount {
        account_id: env::current_account_id(),
        staked_balance: staked.into(),
        unstaked_balance: unstaked.into(),
        can_withdraw: true,
    }
}

#[test]
fn loss_updates_price_and_isolates_only_affected_validator() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);
    let supply = contract.data().token.total_supply;

    contract.reconcile_reward_balance(validator, account(25 * ONE_NEAR, 20 * ONE_NEAR));

    let updated = contract
        .data()
        .validator_pool
        .get_validator(&"validator.test".parse().unwrap())
        .unwrap();
    assert_eq!(updated.staked_amount, 25 * ONE_NEAR);
    assert_eq!(updated.unstaked_amount, 20 * ONE_NEAR);
    assert_eq!(updated.weight, 0);
    assert_eq!(updated.base_stake_amount, 0);
    assert!(!updated.executing);
    assert_eq!(contract.data().validator_pool.total_weight, 0);
    assert_eq!(contract.data().validator_pool.total_base_stake_amount, 0);
    assert_eq!(contract.data().total_staked_near_amount, 45 * ONE_NEAR);
    assert_eq!(contract.data().token.total_supply, supply);
    assert_eq!(contract.ft_price().0, 9 * ONE_NEAR / 10);
}

#[test]
fn loss_below_old_unstaked_balance_uses_reported_components() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);

    contract.reconcile_reward_balance(validator, account(15 * ONE_NEAR, 0));

    let updated = contract
        .data()
        .validator_pool
        .get_validator(&"validator.test".parse().unwrap())
        .unwrap();
    assert_eq!(updated.staked_amount, 15 * ONE_NEAR);
    assert_eq!(updated.unstaked_amount, 0);
    assert_eq!(contract.data().total_staked_near_amount, 15 * ONE_NEAR);
}

#[test]
fn one_yocto_loss_is_not_treated_as_reward() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);
    let supply = contract.data().token.total_supply;

    contract.reconcile_reward_balance(validator, account(30 * ONE_NEAR - 1, 20 * ONE_NEAR));

    assert_eq!(contract.data().total_staked_near_amount, 50 * ONE_NEAR - 1);
    assert_eq!(contract.data().token.total_supply, supply);
    assert_eq!(
        contract
            .data()
            .validator_pool
            .get_validator(&"validator.test".parse().unwrap())
            .unwrap()
            .weight,
        0
    );
}

#[test]
fn equal_total_syncs_components_without_minting_rewards() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);
    let supply = contract.data().token.total_supply;

    contract.reconcile_reward_balance(validator.clone(), account(25 * ONE_NEAR, 25 * ONE_NEAR));

    let updated = contract
        .data()
        .validator_pool
        .get_validator(&validator.account_id)
        .unwrap();
    assert_eq!(updated.staked_amount, 25 * ONE_NEAR);
    assert_eq!(updated.unstaked_amount, 25 * ONE_NEAR);
    assert_eq!(updated.weight, 10);
    assert!(!updated.executing);
    assert_eq!(contract.data().total_staked_near_amount, 50 * ONE_NEAR);
    assert_eq!(contract.data().token.total_supply, supply);
}

#[test]
fn failed_reward_query_releases_the_execution_lock() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);

    contract
        .validator_reward_account_callback(validator.account_id.clone(), Err(PromiseError::Failed));

    assert!(
        !contract
            .data()
            .validator_pool
            .get_validator(&validator.account_id)
            .unwrap()
            .executing
    );
    assert!(test_utils::get_logs()
        .last()
        .unwrap()
        .contains("\"reason\":\"validator_call_failed\""));
}

#[test]
fn legacy_balance_callback_releases_the_execution_lock() {
    let (mut contract, validator) = setup(50 * ONE_NEAR);
    contract
        .validator_get_balance_callback(validator.account_id.clone(), Ok((45 * ONE_NEAR).into()));

    assert!(
        !contract
            .data()
            .validator_pool
            .get_validator(&validator.account_id)
            .unwrap()
            .executing
    );
    assert_eq!(contract.data().total_staked_near_amount, 50 * ONE_NEAR);
    assert!(test_utils::get_logs()
        .last()
        .unwrap()
        .contains("\"reason\":\"legacy_callback_retry\""));
}

#[test]
fn uncovered_loss_does_not_block_other_reward_sync() {
    let (mut contract, validator) = setup(2 * ONE_NEAR);

    contract.reconcile_reward_balance(validator, account(25 * ONE_NEAR, 20 * ONE_NEAR));

    assert_eq!(contract.data().total_staked_near_amount, 0);
    assert_eq!(contract.ft_price().0, 0);
    assert!(
        !contract
            .data()
            .validator_pool
            .get_validator(&"validator.test".parse().unwrap())
            .unwrap()
            .executing
    );

    let mut healthy = Validator::new("healthy.test".parse().unwrap(), 1);
    healthy.executing = true;
    contract.data_mut().validator_pool.total_weight += healthy.weight;
    contract.data_mut().validator_pool.save_validator(&healthy);
    contract.reconcile_reward_balance(healthy.clone(), account(0, 0));
    assert_eq!(contract.data().total_staked_near_amount, 0);
    let mut healthy = contract
        .data()
        .validator_pool
        .get_validator(&healthy.account_id)
        .unwrap();
    assert!(!healthy.executing);
    healthy.executing = true;
    contract.data_mut().validator_pool.save_validator(&healthy);
    contract.reconcile_reward_balance(healthy, account(ONE_NEAR, 0));
    assert_eq!(contract.data().total_staked_near_amount, ONE_NEAR);
    assert!(test_utils::get_logs()
        .last()
        .unwrap()
        .contains("\"event\":\"epoch_update_rewards\""));
    assert!(
        !contract
            .data()
            .validator_pool
            .get_validator(&"healthy.test".parse().unwrap())
            .unwrap()
            .executing
    );
}

#[test]
fn epoch_selection_skips_zero_weight_validator() {
    let (mut contract, mut isolated) = setup(50 * ONE_NEAR);
    isolated.weight = 0;
    isolated.base_stake_amount = 0;
    isolated.unstaked_amount = 0;
    isolated.executing = false;
    contract.data_mut().validator_pool.total_weight = 1;
    contract.data_mut().validator_pool.total_base_stake_amount = 0;
    contract.data_mut().validator_pool.save_validator(&isolated);

    let mut healthy = Validator::new("healthy.test".parse().unwrap(), 1);
    healthy.staked_amount = 5 * ONE_NEAR;
    contract.data_mut().validator_pool.save_validator(&healthy);

    let candidate = contract
        .data()
        .validator_pool
        .get_candidate_to_unstake_v2(7 * ONE_NEAR, 0)
        .unwrap();
    assert_eq!(candidate.validator.account_id, healthy.account_id);
    assert_eq!(candidate.amount, 5 * ONE_NEAR);
    assert_eq!(
        contract
            .data()
            .validator_pool
            .get_num_epoch_to_unstake(7 * ONE_NEAR),
        2 * NUM_EPOCHS_TO_UNLOCK
    );
}

#[test]
fn isolated_balance_does_not_block_healthy_unstake() {
    let (mut contract, mut isolated) = setup(55 * ONE_NEAR);
    isolated.weight = 0;
    isolated.base_stake_amount = 0;
    isolated.staked_amount = 25 * ONE_NEAR;
    isolated.unstaked_amount = 0;
    isolated.executing = false;
    contract.data_mut().validator_pool.total_weight = 10;
    contract.data_mut().validator_pool.total_base_stake_amount = 0;
    contract.data_mut().validator_pool.save_validator(&isolated);

    let mut healthy = Validator::new("healthy.test".parse().unwrap(), 10);
    healthy.staked_amount = 30 * ONE_NEAR;
    contract.data_mut().validator_pool.save_validator(&healthy);

    let candidate = contract
        .data()
        .validator_pool
        .get_candidate_to_unstake_v2(10 * ONE_NEAR, 45 * ONE_NEAR)
        .unwrap();
    assert_eq!(candidate.validator.account_id, healthy.account_id);
    assert_eq!(candidate.amount, 10 * ONE_NEAR);
}

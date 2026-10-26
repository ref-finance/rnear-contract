mod setup;
use setup::*;

const ERR_PERM: &str = "Smart contract panicked: Insufficient permissions for method";

// cargo nextest run --package lst --test test_drain --no-capture

async fn summary(context: &Context) -> lst::Summary {
    context.lst_contract.0.call("get_summary").view().await.unwrap().json().unwrap()
}

#[tokio::test]
async fn test_drain_not_manager() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    
    check!(context.lst_contract.acl_grant_role(&context.root, "OpManager".to_string(), context.manager.id()));

    check!(context.lst_contract.drain_unstake(&context.alice, context.bob.id()), ERR_PERM);
}

#[tokio::test]
async fn test_drain_constraints() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    // add validator
    let v1 = context.create_validator("v1").await;
    check!(context.lst_contract.add_validator(&context.root, v1.0.id(), 10));

    // update base stake amount to 20 NEAR
    check!(context.lst_contract.acl_grant_role(&context.root, "OpManager".to_string(), context.manager.id()));
    check!(context.lst_contract.update_base_stake_amounts(&context.manager, vec![v1.0.id()], vec![NearToken::from_near(20).as_yoctonear()]));

    // user stake
    check!(context.lst_contract.storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));

    // run stake
    context.op_epoch_stake_all().await;

    // 1. cannot drain unstake when weight > 0
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()), lst::ERR_NON_ZERO_WEIGHT);

    // set weight to 0
    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 0));

    // 2. cannot drain unstake when base stake amount > 0
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()), lst::ERR_NON_ZERO_BASE_STAKE_AMOUNT);

    // update base stake amount to 0 NEAR
    check!(context.lst_contract.update_base_stake_amounts(&context.root, vec![v1.0.id()], vec![0_u128]));

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 11));

    // user unstake
    check!(context.lst_contract.unstake_all(&context.alice));

    context.op_epoch_unstake_all().await;

    // validator now have unstaked balance > 0
    context.check_validator_amount(&v1, NearToken::from_near(10).as_yoctonear(), NearToken::from_near(50).as_yoctonear(), None, None).await;

    // -- 3. cannot drain unstake when pending release
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()), lst::ERR_VALIDATOR_UNSTAKE_WHEN_LOCKED);

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 15));

    // -- 4. cannot drain unstake when unstaked balance > 0
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()), lst::ERR_BAD_UNSTAKED_AMOUNT);
}

#[tokio::test]
async fn test_drain_unstake_withdraw() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    // add validator
    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    check!(context.lst_contract.add_validator(&context.root, v1.0.id(), 10));
    check!(context.lst_contract.add_validator(&context.root, v2.0.id(), 10));

    // update base stake amount of v1 to 20 NEAR
    check!(context.lst_contract.acl_grant_role(&context.root, "OpManager".to_string(), context.manager.id()));
    check!(context.lst_contract.update_base_stake_amounts(&context.manager, vec![v1.0.id()], vec![NearToken::from_near(20).as_yoctonear()]));

    // user stake
    check!(context.lst_contract.storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));

    // run stake
    context.op_epoch_stake_all().await;

    // **************************
    // Steps to drain a validator
    // 1. set weight to 0
    // 2. set base stake amount to 0
    // 3. call drain_unstake
    // 4. call drain_withdraw
    // **************************

    // set weight to 0
    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 0));

    // update base stake amount to 0 NEAR
    check!(context.lst_contract.update_base_stake_amounts(&context.root, vec![v1.0.id()], vec![0_u128]));

    // drain_unstake from v1
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()));

    // make sure the validator is in draining mode
    assert_eq!(context.lst_contract.get_validator(v1.0.id()).await.unwrap().unwrap().draining, true);

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 14));

    // epoch_withdraw should not be allowed
    check!(context.lst_contract.epoch_withdraw(&context.bob, v1.0.id()), lst::ERR_DRAINING);

    // drain_withdraw from v1
    check!(context.lst_contract.drain_withdraw(&context.root, v1.0.id()));

    // make sure v1 is drained
    assert_eq!(context.lst_contract.get_validator(v1.0.id()).await.unwrap().unwrap().draining, false);
    context.check_validator_amount(&v1, 0, 0, None, None).await;
    context.check_validator_amount(&v2, NearToken::from_near(20).as_yoctonear(), 0, None, None).await;

    // restake and make sure funds are re-distributed
    context.op_epoch_stake_all().await;
    context.check_validator_amount(&v1, 0, 0, None, None).await;
    context.check_validator_amount(&v2, NearToken::from_near(60).as_yoctonear(), 0, None, None).await;
}

#[tokio::test]
async fn test_drain_get_account_fail() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    // add validator
    let v1 = context.create_validator("v1").await;
    check!(context.lst_contract.add_validator(&context.root, v1.0.id(), 10));

    // user stake
    check!(context.lst_contract.storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));

    // run stake
    context.op_epoch_stake_all().await;

    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 0));

    check!(v1.set_get_account_fail(&context.root, true));
    check!(logs context.lst_contract.drain_unstake(&context.root, v1.0.id()), "get_account() failed, for testing purpose");
}

#[tokio::test]
async fn test_drain_unstake_settles_rewards() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(context.mock_whitelist.allow_all(&context.root));

    let validator = context.create_validator("v1").await;
    check!(context.lst_contract.add_validator(&context.root, validator.0.id(), 10));
    context.op_epoch_stake_all().await;
    check!(context.lst_contract.update_weight(&context.root, validator.0.id(), 0));

    for index in 0..10 {
        let beneficiary: AccountId = format!("fee{index}.near").parse().unwrap();
        check!(context.lst_contract.set_beneficiary(&context.root, &beneficiary, 100));
    }

    let reward = 2 * lst::ONE_NEAR;
    let before = summary(&context).await;
    check!(validator.set_pending_reward(&context.root, reward));
    check!(context.lst_contract.drain_unstake(&context.root, validator.0.id()));

    context.check_validator_amount(&validator, 0, 12 * lst::ONE_NEAR, None, None).await;
    let after = summary(&context).await;
    assert_eq!(after.total_staked_near_amount.0, before.total_staked_near_amount.0 + reward);
    assert!(after.total_share_amount.0 > before.total_share_amount.0);
    assert_eq!(after.epoch_requested_stake_amount.0, before.epoch_requested_stake_amount.0);

    check!(context.lst_contract.epoch_update_rewards(&context.root, validator.0.id()));
    let repeated = summary(&context).await;
    assert_eq!(repeated.total_staked_near_amount.0, after.total_staked_near_amount.0);
    assert_eq!(repeated.total_share_amount.0, after.total_share_amount.0);

    check!(context.lst_contract.set_epoch_height(&context.root, 14));
    check!(context.lst_contract.drain_withdraw(&context.root, validator.0.id()));
    context.check_validator_amount(&validator, 0, 0, None, None).await;
    let withdrawn = summary(&context).await;
    assert_eq!(withdrawn.epoch_requested_stake_amount.0, after.epoch_requested_stake_amount.0 + 12 * lst::ONE_NEAR);
}

#[tokio::test]
async fn test_drain_retry_recovers_staked_residue() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(context.mock_whitelist.allow_all(&context.root));

    let validator = context.create_validator("v1").await;
    check!(context.lst_contract.add_validator(&context.root, validator.0.id(), 10));
    context.op_epoch_stake_all().await;
    check!(context.lst_contract.update_weight(&context.root, validator.0.id(), 0));
    check!(context.lst_contract.drain_unstake(&context.root, validator.0.id()));

    let residue = 2 * lst::ONE_NEAR;
    check!(validator.add_reward(context.lst_contract.0.as_account(), residue));
    let premature_sync = context.lst_contract.sync_balance_from_validator(&context.root, validator.0.id()).await.unwrap();
    assert!(premature_sync.logs().iter().any(|log| log.contains("sync_validator_balance_failed_large_diff")));
    check!(context.lst_contract.epoch_update_rewards(&context.root, validator.0.id()));
    context.check_validator_amount(&validator, residue, 10 * lst::ONE_NEAR, None, None).await;

    check!(context.lst_contract.set_epoch_height(&context.root, 14));
    check!(context.lst_contract.drain_withdraw(&context.root, validator.0.id()), lst::ERR_NON_ZERO_STAKED_AMOUNT);
    check!(validator.set_panic(&context.root, true));
    check!(logs context.lst_contract.drain_unstake(&context.root, validator.0.id()), "Test Panic!");
    let after_failure = context.lst_contract.get_validator(validator.0.id()).await.unwrap().unwrap();
    assert!(after_failure.draining);
    assert!(!after_failure.pending_release);
    check!(validator.set_panic(&context.root, false));
    context.check_validator_amount(&validator, residue, 10 * lst::ONE_NEAR, None, None).await;

    check!(validator.set_pending_reward(&context.root, lst::ONE_NEAR));
    let before_retry = summary(&context).await;
    check!(context.lst_contract.drain_unstake(&context.root, validator.0.id()));
    context.check_validator_amount(&validator, 0, 13 * lst::ONE_NEAR, None, None).await;
    let after_retry = summary(&context).await;
    assert_eq!(after_retry.total_staked_near_amount.0, before_retry.total_staked_near_amount.0 + lst::ONE_NEAR);
    assert_eq!(after_retry.epoch_requested_stake_amount.0, before_retry.epoch_requested_stake_amount.0);

    check!(context.lst_contract.set_epoch_height(&context.root, 18));
    check!(context.lst_contract.drain_withdraw(&context.root, validator.0.id()));
    context.check_validator_amount(&validator, 0, 0, None, None).await;
    let withdrawn = summary(&context).await;
    assert_eq!(withdrawn.epoch_requested_stake_amount.0, before_retry.epoch_requested_stake_amount.0 + 13 * lst::ONE_NEAR);
}

#[tokio::test]
async fn test_drain_sync_recovers_after_query_failure() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(context.mock_whitelist.allow_all(&context.root));

    let validator = context.create_validator("v1").await;
    check!(context.lst_contract.add_validator(&context.root, validator.0.id(), 10));
    context.op_epoch_stake_all().await;
    check!(context.lst_contract.update_weight(&context.root, validator.0.id(), 0));

    check!(validator.set_pending_reward(&context.root, 2 * lst::ONE_NEAR));
    check!(validator.set_get_account_fail(&context.root, true));
    check!(logs context.lst_contract.drain_unstake(&context.root, validator.0.id()), "get_account() failed, for testing purpose");

    let before_sync = context.lst_contract.get_validator(validator.0.id()).await.unwrap().unwrap();
    assert!(before_sync.draining);
    assert_eq!(before_sync.unstaked_amount.0, 10 * lst::ONE_NEAR);
    assert_eq!(validator.get_account_unstaked_balance(context.lst_contract.0.id()).await.unwrap().0, 12 * lst::ONE_NEAR);

    check!(validator.set_get_account_fail(&context.root, false));
    check!(context.lst_contract.sync_balance_from_validator(&context.root, validator.0.id()));
    context.check_validator_amount(&validator, 0, 12 * lst::ONE_NEAR, None, None).await;
    assert_eq!(summary(&context).await.total_staked_near_amount.0, 12 * lst::ONE_NEAR);

    check!(context.lst_contract.set_epoch_height(&context.root, 14));
    check!(context.lst_contract.drain_withdraw(&context.root, validator.0.id()));
}

mod setup;
use setup::*;

const ERR_PERM: &str = "Smart contract panicked: Insufficient permissions for method";

// cargo nextest run --package lst --test test_drain --no-capture

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

    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 0));

    // 2. cannot drain unstake when base stake amount > 0
    check!(context.lst_contract.drain_unstake(&context.root, v1.0.id()), lst::ERR_NON_ZERO_BASE_STAKE_AMOUNT);

    // update base stake amount to 0 NEAR
    check!(context.lst_contract.update_base_stake_amounts(&context.root, vec![v1.0.id()], vec![0_u128]));

    // Restore eligibility for normal epoch unstake.
    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 10));

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 11));

    // user unstake
    check!(context.lst_contract.unstake_all(&context.alice));

    context.op_epoch_unstake_all().await;

    // validator now have unstaked balance > 0
    context.check_validator_amount(&v1, NearToken::from_near(10).as_yoctonear(), NearToken::from_near(50).as_yoctonear(), None, None).await;

    // set weight to 0 after normal unstake; zero-weight validators are excluded from epoch selection
    check!(context.lst_contract.update_weight(&context.root, v1.0.id(), 0));

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

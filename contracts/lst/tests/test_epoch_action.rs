mod setup;
use setup::*;

#[tokio::test]
async fn test_epoch_stake() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    let v3 = context.create_validator("v3").await;

    check!(context
        .lst_contract
        .add_validator(&context.root, v1.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, v2.0.id(), 20));
    check!(context
        .lst_contract
        .add_validator(&context.root, v3.0.id(), 30));

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));

    // at this time there should be no NEAR actually staked on validators
    context.check_validator_amount(&v1, 0, 0, None, None).await;
    context.check_validator_amount(&v2, 0, 0, None, None).await;
    context.check_validator_amount(&v3, 0, 0, None, None).await;

    context.op_epoch_stake_all().await;

    // validators should have staked balance based on their weights
    // note that 10 NEAR is already staked when contract init
    context
        .check_validator_amount(&v1, NearToken::from_near(10).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(&v2, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(&v3, NearToken::from_near(30).as_yoctonear(), 0, None, None)
        .await;

    // pass one epoch
    check!(context.lst_contract.set_epoch_height(&context.root, 11));
    check!(view context.lst_contract.read_epoch_height());

    // stake more
    check!(context
        .lst_contract
        .storage_deposit(&context.bob, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.bob, 90));

    context.op_epoch_stake_all().await;

    // validators should have staked balance based on their weights
    // note that 10 NEAR is already staked when contract init
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(10 + 15).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(20 + 30).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(30 + 45).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;

    // ---- Test base stake amount ----

    // set manager
    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.lst_contract.update_base_stake_amounts(
        &context.manager,
        vec![v1.0.id()],
        vec![NearToken::from_near(20).as_yoctonear()]
    ));

    // pass one epoch
    check!(context.lst_contract.set_epoch_height(&context.root, 12));

    // stake more
    check!(context.lst_contract.deposit_and_stake(&context.bob, 50));

    context.op_epoch_stake_all().await;

    // validators should have staked balance based on their weights + base stake amounts
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(10 + 15 + 25).as_yoctonear(),
            0,
            Some(NearToken::from_near(20).as_yoctonear()),
            Some(NearToken::from_near(50).as_yoctonear()),
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(20 + 30 + 10).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(30 + 45 + 15).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
}

#[tokio::test]
async fn test_epoch_stake_with_rounding_diff() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    let v3 = context.create_validator("v3").await;

    check!(v1.set_balance_delta(&context.root, 1, 1));

    check!(context
        .lst_contract
        .add_validator(&context.root, v1.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, v2.0.id(), 20));
    check!(context
        .lst_contract
        .add_validator(&context.root, v3.0.id(), 30));

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));
    context.op_epoch_stake_all().await;
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(10).as_yoctonear() - 1,
            1,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(&v2, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(&v3, NearToken::from_near(30).as_yoctonear(), 0, None, None)
        .await;

    check!(context.lst_contract.set_epoch_height(&context.root, 11));

    // stake more
    check!(context
        .lst_contract
        .storage_deposit(&context.bob, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.bob, 90));

    context.op_epoch_stake_all().await;

    // validators should have staked balance based on their weights
    // note that 10 NEAR is already staked when contract init
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(10 + 15).as_yoctonear() - 2,
            2,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(20 + 30).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(30 + 45).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;

    // ---- Test base stake amount ----

    // set manager
    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.lst_contract.update_base_stake_amounts(
        &context.manager,
        vec![v1.0.id()],
        vec![NearToken::from_near(20).as_yoctonear()]
    ));

    // pass one epoch
    check!(context.lst_contract.set_epoch_height(&context.root, 12));

    // stake more
    check!(context.lst_contract.deposit_and_stake(&context.bob, 50));

    context.op_epoch_stake_all().await;

    // validators should have staked balance based on their weights + base stake amounts
    // - v1 is selected first, and to meet the target amount, 25 N + 2 yN will be staked, which reduced the diff for staked amount
    // - v3 is then selected since its delta is higher than v2, though their delta/target are the same
    // - v2 is finally selected with 2 yN diff which is moved to v1
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(10 + 15 + 25).as_yoctonear() - 1,
            3,
            Some(NearToken::from_near(20).as_yoctonear()),
            Some(NearToken::from_near(50).as_yoctonear()),
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(20 + 30 + 10).as_yoctonear() - 2,
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(30 + 45 + 15).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
}

#[tokio::test]
async fn test_epoch_unstake() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    let v3 = context.create_validator("v3").await;

    check!(context
        .lst_contract
        .add_validator(&context.root, v1.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, v2.0.id(), 20));
    check!(context
        .lst_contract
        .add_validator(&context.root, v3.0.id(), 30));

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 110));
    context.op_epoch_stake_all().await;

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 14));

    check!(context.lst_contract.unstake(&context.alice, 30));

    // at this time no actual unstake should happen
    context
        .check_validator_amount(&v1, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(&v2, NearToken::from_near(40).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(&v3, NearToken::from_near(60).as_yoctonear(), 0, None, None)
        .await;

    context.op_epoch_unstake_all().await;

    // 60 NEAR was initially staked, 30 was taken out
    context
        .check_validator_amount(&v1, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_millinear(32500).as_yoctonear(),
            NearToken::from_millinear(7500).as_yoctonear(),
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_millinear(37500).as_yoctonear(),
            NearToken::from_millinear(22500).as_yoctonear(),
            None,
            None,
        )
        .await;

    // unstake more
    check!(context.lst_contract.unstake(&context.alice, 18));

    // epoch unstake should not take effect now
    context
        .check_validator_amount(&v1, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_millinear(32500).as_yoctonear(),
            NearToken::from_millinear(7500).as_yoctonear(),
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_millinear(37500).as_yoctonear(),
            NearToken::from_millinear(22500).as_yoctonear(),
            None,
            None,
        )
        .await;

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 18));

    context.op_epoch_unstake_all().await;

    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(12).as_yoctonear(),
            NearToken::from_near(8).as_yoctonear(),
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_millinear(22500).as_yoctonear(),
            NearToken::from_millinear(17500).as_yoctonear(),
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_millinear(37500).as_yoctonear(),
            NearToken::from_millinear(22500).as_yoctonear(),
            None,
            None,
        )
        .await;

    // ---- Test base stake amount ----

    // set manager
    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.lst_contract.update_base_stake_amounts(
        &context.manager,
        vec![v1.0.id()],
        vec![NearToken::from_near(10).as_yoctonear()]
    ));

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 22));

    // unstake more; remaining total staked: 120 - 30 - 18 - 26 = 46
    check!(context.lst_contract.unstake(&context.alice, 26));
    context.op_epoch_unstake_all().await;

    // validators should have target stake amount based on weights + base stake amounts
    // - 1st epoch_unstake() unstaked 19.5 NEAR (amount = delta) from validator v3
    // - 2nd epoch_unstake() unstaked 6.5 NEAR (amount = rest) from validator v2
    // target = 10 (base) + 6 (weighted) = 16; delta (1st) = 12 - 16 = -4; delta (2nd) = 12 - 16 = -4;
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(12).as_yoctonear(),
            NearToken::from_near(8).as_yoctonear(),
            Some(NearToken::from_near(10).as_yoctonear()),
            Some(NearToken::from_near(16).as_yoctonear()),
        )
        .await;
    // target = 12 (weighted); delta (1st) = 22.5 - 12 = 10.5; delta (2nd) = 16 - 12 = 4;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(16).as_yoctonear(),
            NearToken::from_near(24).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            Some(NearToken::from_near(12).as_yoctonear()),
        )
        .await;
    // target = 18 (weighted); delta (1st) = 37.5 - 18 = 19.5; delta (2nd) = 18 - 18 = 0;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(18).as_yoctonear(),
            NearToken::from_near(42).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            Some(NearToken::from_near(18).as_yoctonear()),
        )
        .await;

    // reset base stake amount of v1 to 0
    check!(context.lst_contract.update_base_stake_amounts(
        &context.manager,
        vec![v1.0.id()],
        vec![NearToken::from_near(0).as_yoctonear()]
    ));

    // pass one epoch
    check!(context.lst_contract.set_epoch_height(&context.root, 26));

    // unstake more; remaining total staked: 120 - 30 - 18 - 26 - 10 = 36
    check!(context.lst_contract.unstake(&context.alice, 10));
    context.op_epoch_unstake_all().await;

    // validators should have target stake amount based on weights + base stake amounts
    // - 1st epoch_unstake() unstaked 6 NEAR (amount = delta) from validator v1;
    // - 2nd epoch_unstake() unstaked 4 NEAR (amount = rest) from validator v2;
    // target = 6 (weighted); delta (1st) = 12 - 6 = 6; delta (2nd) = 6 - 6 = 0;
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(6).as_yoctonear(),
            NearToken::from_near(14).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            Some(NearToken::from_near(6).as_yoctonear()),
        )
        .await;
    // target = 12 (weighted); delta (1st) = 16 - 12 = 4; delta (2nd) = 12 - 12 = 0;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(12).as_yoctonear(),
            NearToken::from_near(28).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            Some(NearToken::from_near(12).as_yoctonear()),
        )
        .await;
    // target = 18 (weighted); delta (1st) = 18 - 18 = 0; delta (2nd) = 18 - 18 = 0;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(18).as_yoctonear(),
            NearToken::from_near(42).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            Some(NearToken::from_near(18).as_yoctonear()),
        )
        .await;
}

#[tokio::test]
async fn test_epoch_rewards() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    let v3 = context.create_validator("v3").await;

    check!(context
        .lst_contract
        .add_validator(&context.root, v1.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, v2.0.id(), 20));
    check!(context
        .lst_contract
        .add_validator(&context.root, v3.0.id(), 30));

    // set manager
    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.lst_contract.update_base_stake_amounts(
        &context.manager,
        vec![v1.0.id()],
        vec![NearToken::from_near(10).as_yoctonear()]
    ));

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 60));
    context.op_epoch_stake_all().await;

    let total_share_balance = context.lst_contract.ft_total_supply().await.unwrap().0;
    let total_staked_balance = context
        .lst_contract
        .get_total_staked_balance()
        .await
        .unwrap()
        .0;
    assert_eq!(total_share_balance, NearToken::from_near(70).as_yoctonear());
    assert_eq!(
        total_staked_balance,
        NearToken::from_near(70).as_yoctonear()
    );

    // generate rewards
    check!(v1.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(2).as_yoctonear()
    ));
    check!(v2.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(2).as_yoctonear()
    ));
    check!(v3.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(3).as_yoctonear()
    ));

    // update rewards
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, v1.0.id()));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, v2.0.id()));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, v3.0.id()));

    let total_share_balance_1 = context.lst_contract.ft_total_supply().await.unwrap().0;
    let total_staked_balance_1 = context
        .lst_contract
        .get_total_staked_balance()
        .await
        .unwrap()
        .0;
    assert_eq!(
        total_share_balance_1,
        NearToken::from_near(70).as_yoctonear()
    );
    assert_eq!(
        total_staked_balance_1,
        NearToken::from_near(77).as_yoctonear()
    );

    // check staked amount and base stake amount on each validator
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(22).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(11).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(22).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(33).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;

    // set beneficiary
    check!(context
        .lst_contract
        .set_beneficiary(&context.root, context.manager.id(), 1000));

    // generate more rewards
    check!(v1.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(2).as_yoctonear()
    ));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, v1.0.id()));

    let total_share_balance_2 = context.lst_contract.ft_total_supply().await.unwrap().0;
    let total_staked_balance_2 = context
        .lst_contract
        .get_total_staked_balance()
        .await
        .unwrap()
        .0;
    assert_eq!(total_share_balance_2, 70_177215189873417721518987);
    assert_eq!(
        total_staked_balance_2,
        NearToken::from_near(79).as_yoctonear()
    );

    // check staked amount and base stake amount on each validator
    context
        .check_validator_amount(
            &v1,
            NearToken::from_near(24).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(12).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(22).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(33).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 14));
    check!(view context.lst_contract.get_account_total_balance(context.alice.id()));

    // unstake 67.5 NEAR; remaining total staked: 79 - 67.5 = 11.5
    check!(context.lst_contract.unstake_in_yocto(
        &context.alice,
        NearToken::from_millinear(67500).as_yoctonear()
    ));
    context.op_epoch_unstake_all().await;

    // check staked amount and base stake amount on each validator
    // TODO: There're 1yN diff due to rounding when alice unstakes 67.5 NEAR
    context
        .check_validator_amount(
            &v1,
            NearToken::from_millinear(11500).as_yoctonear(),
            NearToken::from_millinear(12500).as_yoctonear(),
            Some(NearToken::from_near(12).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(0).as_yoctonear(),
            NearToken::from_near(22).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(0).as_yoctonear(),
            NearToken::from_near(33).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;

    // fast-forward 4 epoch
    check!(context.lst_contract.set_epoch_height(&context.root, 18));

    // withdraw again
    check!(context
        .lst_contract
        .epoch_withdraw(&context.root, v2.0.id()));

    // check staked amount and base stake amount on each validator
    context
        .check_validator_amount(
            &v1,
            NearToken::from_millinear(11500).as_yoctonear(),
            NearToken::from_millinear(12500).as_yoctonear(),
            Some(NearToken::from_near(12).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(0).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(0).as_yoctonear(),
            NearToken::from_near(33).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;

    // generate more rewards
    check!(v2.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(2).as_yoctonear()
    ));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, v2.0.id()));

    // check staked amount and base stake amount on each validator
    context
        .check_validator_amount(
            &v1,
            NearToken::from_millinear(11500).as_yoctonear(),
            NearToken::from_millinear(12500).as_yoctonear(),
            Some(NearToken::from_near(12).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_near(2).as_yoctonear(),
            NearToken::from_near(0).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_near(0).as_yoctonear(),
            NearToken::from_near(33).as_yoctonear(),
            Some(NearToken::from_near(0).as_yoctonear()),
            None,
        )
        .await;
}

#[tokio::test]
async fn test_epoch_withdraw() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(view context.lst_contract.get_version());
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1 = context.create_validator("v1").await;
    let v2 = context.create_validator("v2").await;
    let v3 = context.create_validator("v3").await;

    check!(context
        .lst_contract
        .add_validator(&context.root, v1.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, v2.0.id(), 20));
    check!(context
        .lst_contract
        .add_validator(&context.root, v3.0.id(), 30));

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 110));
    context.op_epoch_stake_all().await;

    // fast-forward
    check!(context.lst_contract.set_epoch_height(&context.root, 11));

    // user unstake
    check!(context.lst_contract.unstake(&context.alice, 30));

    // epoch unstake
    context.op_epoch_unstake_all().await;

    context
        .check_validator_amount(&v1, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_millinear(32500).as_yoctonear(),
            NearToken::from_millinear(7500).as_yoctonear(),
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_millinear(37500).as_yoctonear(),
            NearToken::from_millinear(22500).as_yoctonear(),
            None,
            None,
        )
        .await;

    // withdraw should fail now
    check!(
        context
            .lst_contract
            .epoch_withdraw(&context.root, v2.0.id()),
        "Cannot withdraw from a pending release validator"
    );
    check!(
        context
            .lst_contract
            .epoch_withdraw(&context.root, v3.0.id()),
        "Cannot withdraw from a pending release validator"
    );

    // fast-forward 4 epochs
    check!(context.lst_contract.set_epoch_height(&context.root, 15));
    // withdraw again
    check!(context
        .lst_contract
        .epoch_withdraw(&context.root, v2.0.id()));
    check!(context
        .lst_contract
        .epoch_withdraw(&context.root, v3.0.id()));

    context
        .check_validator_amount(&v1, NearToken::from_near(20).as_yoctonear(), 0, None, None)
        .await;
    context
        .check_validator_amount(
            &v2,
            NearToken::from_millinear(32500).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
    context
        .check_validator_amount(
            &v3,
            NearToken::from_millinear(37500).as_yoctonear(),
            0,
            None,
            None,
        )
        .await;
}

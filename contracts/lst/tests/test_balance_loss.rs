mod setup;
use setup::*;

#[tokio::test]
async fn validator_loss_is_isolated_and_other_rewards_continue() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    check!(context.mock_whitelist.allow_all(&context.root));

    let slashed = context.create_validator("slashed").await;
    let healthy = context.create_validator("healthy").await;
    check!(context
        .lst_contract
        .add_validator(&context.root, slashed.0.id(), 10));
    check!(context
        .lst_contract
        .add_validator(&context.root, healthy.0.id(), 10));
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit_and_stake(&context.alice, 50));
    context.op_epoch_stake_all().await;

    let before = context
        .lst_contract
        .get_validator(slashed.0.id())
        .await
        .unwrap()
        .unwrap();
    let share_supply = context.lst_contract.ft_total_supply().await.unwrap().0;
    check!(slashed.slash(
        &context.root,
        context.lst_contract.0.id(),
        NearToken::from_near(5).as_yoctonear(),
        0,
    ));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, slashed.0.id()));

    let after = context
        .lst_contract
        .get_validator(slashed.0.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.weight, 0);
    assert_eq!(after.base_stake_amount.0, 0);
    assert_eq!(
        after.staked_amount.0,
        before.staked_amount.0 - NearToken::from_near(5).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_total_staked_balance()
            .await
            .unwrap()
            .0,
        NearToken::from_near(55).as_yoctonear()
    );
    assert_eq!(
        context.lst_contract.ft_total_supply().await.unwrap().0,
        share_supply
    );

    check!(healthy.add_reward(
        context.lst_contract.0.as_account(),
        NearToken::from_near(1).as_yoctonear()
    ));
    check!(context
        .lst_contract
        .epoch_update_rewards(&context.root, healthy.0.id()));
    assert_eq!(
        context
            .lst_contract
            .get_total_staked_balance()
            .await
            .unwrap()
            .0,
        NearToken::from_near(56).as_yoctonear()
    );

    check!(context.lst_contract.unstake(&context.alice, 20));
    check!(context.lst_contract.set_epoch_height(&context.root, 11));
    context.op_epoch_unstake_all().await;
    assert_eq!(
        healthy
            .get_account_staked_balance(context.lst_contract.0.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(11).as_yoctonear()
    );
    check!(context
        .lst_contract
        .drain_unstake(&context.root, slashed.0.id()));
}

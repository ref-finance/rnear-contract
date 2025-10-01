mod setup;
use setup::*;

#[tokio::test]
async fn test_account_initial_balance() {
    // cargo test --package lst --test test_staking_pool_itf -- test_account_initial_balance --exact --show-output
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        0
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        0
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_total_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        0
    );
}

#[tokio::test]
async fn test_account_deposit_zero() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(
        context.lst_contract.deposit(&context.alice, 0),
        "Deposit amount should be positive"
    );
}

#[tokio::test]
async fn test_account_deposit_then_stake() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    // deposit
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit(&context.alice, 10));
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10).as_yoctonear()
    );

    // stake
    let outcome = context.lst_contract.stake(&context.alice, 9).await.unwrap();
    assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
    let lst_amount = outcome.json::<U128>().unwrap().0;
    assert_eq!(lst_amount, NearToken::from_near(9).as_yoctonear());
    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(9).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10 - 9).as_yoctonear()
    );

    // stake all
    let outcome = context
        .lst_contract
        .stake_all(&context.alice)
        .await
        .unwrap();
    assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
    let lst_amount = outcome.json::<U128>().unwrap().0;
    assert_eq!(lst_amount, NearToken::from_near(1).as_yoctonear());
    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );
}

#[tokio::test]
async fn test_account_deposit_and_stake() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    // depoist and stake
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    let outcome = context
        .lst_contract
        .deposit_and_stake(&context.alice, 10)
        .await
        .unwrap();
    assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
    let lst_amount = outcome.json::<U128>().unwrap().0;
    assert_eq!(lst_amount, NearToken::from_near(10).as_yoctonear());
    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );
}

#[tokio::test]
async fn test_account_unstake() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    // deposit
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit(&context.alice, 10));

    // stake
    check!(context.lst_contract.stake(&context.alice, 9));

    // unstake
    check!(context.lst_contract.unstake(&context.alice, 5));

    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(9 - 5).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10 - 9 + 5).as_yoctonear()
    );
}

#[tokio::test]
async fn test_account_unstake_and_withdraw() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    // deposit
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit(&context.alice, 10));

    // stake
    check!(context.lst_contract.stake(&context.alice, 8));

    // first withdraw
    check!(context.lst_contract.withdraw(&context.alice, 1));

    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(8).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(10 - 8 - 1).as_yoctonear()
    );

    // unstake
    check!(context.lst_contract.unstake(&context.alice, 5));

    // withdraw all immediately, should fail
    check!(
        context.lst_contract.withdraw_all(&context.alice),
        "The unstaked balance is not yet available due to unstaking delay"
    );

    // wait 4 epochs
    context.epoch_height_fast_forward(None).await;

    // withdraw all after 4 epochs
    check!(context.lst_contract.withdraw_all(&context.alice));

    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(3).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );

    // unstake all
    check!(context.lst_contract.unstake_all(&context.alice));
    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(3).as_yoctonear()
    );

    // wait 4 epochs
    context.epoch_height_fast_forward(None).await;

    // withdraw all remaining after 4 epochs
    check!(context.lst_contract.withdraw(&context.alice, 3));

    assert_eq!(
        context
            .lst_contract
            .get_account_staked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );
    assert_eq!(
        context
            .lst_contract
            .get_account_unstaked_balance(context.alice.id())
            .await
            .unwrap()
            .0,
        NearToken::from_near(0).as_yoctonear()
    );
}

#[tokio::test]
async fn test_account_late_unstake_and_withdraw() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    // deposit
    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit(&context.alice, 10));

    // stake
    check!(context.lst_contract.stake(&context.alice, 9));

    // call epoch_stake, in order to trigger clean up
    context.op_epoch_stake_all().await;

    // unstake
    check!(context.lst_contract.unstake(&context.alice, 5));

    // withdraw available time should be 5 epochs later
    let account_details = context
        .lst_contract
        .get_account_details(context.alice.id())
        .await
        .unwrap();
    assert_eq!(account_details.unstaked_available_epoch_height, 15);

    // cannot withdraw after 4 epochs
    context.epoch_height_fast_forward(None).await;
    check!(
        context.lst_contract.withdraw(&context.alice, 5),
        "The unstaked balance is not yet available due to unstaking delay"
    );

    // wait for one more epoch
    context.epoch_height_fast_forward(Some(1)).await;
    check!(context.lst_contract.withdraw(&context.alice, 5));
}

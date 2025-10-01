mod setup;
use setup::*;

#[tokio::test]
async fn test_ft_metadata() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    let metadata = context.lst_contract.ft_metadata().await.unwrap();
    assert_eq!(metadata.symbol, "rNEAR");
    assert_eq!(metadata.decimals, 24);
}

#[tokio::test]
async fn test_lst_price() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;
    let lst_price = context.lst_contract.ft_price().await.unwrap().0;
    assert_eq!(lst_price, NearToken::from_near(1).as_yoctonear());
}

#[tokio::test]
async fn test_ft_transfer_with_no_balance() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));

    check!(
        context
            .lst_contract
            .ft_transfer(&context.alice, context.bob.id(), 100),
        "The account doesn't have enough balance"
    );
}

#[tokio::test]
async fn test_stake_and_transfer() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context
        .lst_contract
        .storage_deposit(&context.bob, None, FT_STORAGE_DEPOSIT));

    // deposit and stake 10 NEAR
    let stake_amount = NearToken::from_near(10).as_yoctonear();
    check!(context.lst_contract.deposit_and_stake(&context.alice, 10));

    // transfer 2 LST from alice to bob
    let transfer_amount_1 = NearToken::from_near(2).as_yoctonear();
    check!(context
        .lst_contract
        .ft_transfer(&context.alice, context.bob.id(), transfer_amount_1));
    assert_eq!(
        context
            .lst_contract
            .ft_balance_of(context.alice.id())
            .await
            .unwrap()
            .0,
        stake_amount - transfer_amount_1
    );
    assert_eq!(
        context
            .lst_contract
            .ft_balance_of(context.bob.id())
            .await
            .unwrap()
            .0,
        transfer_amount_1
    );

    // transfer 1 LST from bob to alice
    let transfer_amount_2 = NearToken::from_near(1).as_yoctonear();
    check!(context
        .lst_contract
        .ft_transfer(&context.bob, context.alice.id(), transfer_amount_2));
    assert_eq!(
        context
            .lst_contract
            .ft_balance_of(context.alice.id())
            .await
            .unwrap()
            .0,
        stake_amount - transfer_amount_1 + transfer_amount_2
    );
    assert_eq!(
        context
            .lst_contract
            .ft_balance_of(context.bob.id())
            .await
            .unwrap()
            .0,
        transfer_amount_1 - transfer_amount_2
    );

    // cannot transfer 2 LST from bob
    check!(
        context
            .lst_contract
            .ft_transfer(&context.bob, context.alice.id(), transfer_amount_1),
        "The account doesn't have enough balance"
    );
}

#[tokio::test]
async fn test_storage_unregister() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context
        .lst_contract
        .storage_deposit(&context.alice, None, FT_STORAGE_DEPOSIT));
    check!(context
        .lst_contract
        .storage_deposit(&context.bob, None, FT_STORAGE_DEPOSIT));

    // can unregister immediately
    check!(context.lst_contract.storage_unregister(&context.bob, None));
    // can NOT unregister with force is true
    check!(
        context
            .lst_contract
            .storage_unregister(&context.alice, Some(true)),
        "Force unregister is not allowed"
    );

    // can NOT unregister with positive unstake balance
    check!(context
        .lst_contract
        .storage_deposit(&context.bob, None, FT_STORAGE_DEPOSIT));
    check!(context.lst_contract.deposit(&context.bob, 10));
    check!(
        context.lst_contract.storage_unregister(&context.bob, None),
        "Can't unregister the account with the positive unstaked balance"
    );

    // can NOT unregister with LST positive balance
    check!(context.lst_contract.stake(&context.bob, 10));
    check!(context.lst_contract.ft_transfer(
        &context.bob,
        context.alice.id(),
        NearToken::from_near(10).as_yoctonear()
    ));
    check!(
        context
            .lst_contract
            .storage_unregister(&context.alice, None),
        "Can't unregister the account with the positive balance without force"
    );

    // unregister after transfer all LST
    check!(context.lst_contract.storage_unregister(&context.bob, None));
}

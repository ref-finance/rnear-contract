mod setup;
use setup::*;

const ERR_PERM: &str = "Smart contract panicked: Insufficient permissions for method";
const ERR_WHITELIST: &str = "Validator not whitelisted";

#[tokio::test]
async fn test_vpool_not_manager() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));

    check!(
        context
            .lst_contract
            .add_validator(&context.alice, context.bob.id(), 10),
        ERR_PERM
    );
    check!(
        context
            .lst_contract
            .add_validators(&context.alice, vec![context.bob.id()], vec![10]),
        ERR_PERM
    );
    check!(
        context
            .lst_contract
            .remove_validator(&context.alice, context.bob.id()),
        ERR_PERM
    );
    check!(
        context
            .lst_contract
            .update_weight(&context.alice, context.bob.id(), 10),
        ERR_PERM
    );
    check!(
        context
            .lst_contract
            .update_weights(&context.alice, vec![context.bob.id()], vec![10]),
        ERR_PERM
    );
    check!(
        context.lst_contract.update_base_stake_amounts(
            &context.alice,
            vec![context.bob.id()],
            vec![10]
        ),
        ERR_PERM
    );
}

#[tokio::test]
async fn test_vpool_add_validator() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1: AccountId = "foo".parse().unwrap();
    check!(context.lst_contract.add_validator(&context.root, &v1, 10));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 10);
    let v2: AccountId = "bar".parse().unwrap();
    check!(context.lst_contract.add_validator(&context.root, &v2, 20));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 30);
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    let vs_map: HashMap<_, _> = vs
        .iter()
        .map(|x| (x.account_id.to_string(), x.weight))
        .collect();
    assert_eq!(vs_map.get("foo").unwrap(), &10_u16);
    assert_eq!(vs_map.get("bar").unwrap(), &20_u16);
}

#[tokio::test]
async fn test_vpool_bulk_add_few_validators() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.mock_whitelist.allow_all(&context.root));

    let v1: AccountId = "foo".parse().unwrap();
    let v2: AccountId = "bar".parse().unwrap();
    check!(context
        .lst_contract
        .add_validators(&context.root, vec![&v1, &v2], vec![10, 20]));

    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 30);
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    let vs_map: HashMap<_, _> = vs
        .iter()
        .map(|x| (x.account_id.to_string(), x.weight))
        .collect();
    assert_eq!(vs_map.get("foo").unwrap(), &10_u16);
    assert_eq!(vs_map.get("bar").unwrap(), &20_u16);
}

#[tokio::test]
async fn test_vpool_bulk_add_lot_validators() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.mock_whitelist.allow_all(&context.root));

    let validators: Vec<AccountId> = (0..6)
        .map(|x| format!("validator-{}", x).parse().unwrap())
        .collect();
    let weights: Vec<u16> = (0..6).map(|_| 10_u16).collect();
    check!(context.lst_contract.add_validators(
        &context.root,
        validators.iter().collect(),
        weights
    ));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 60);

    let validators: Vec<AccountId> = (6..12)
        .map(|x| format!("validator-{}", x).parse().unwrap())
        .collect();
    let weights: Vec<u16> = (6..12).map(|_| 10_u16).collect();
    check!(context.lst_contract.add_validators(
        &context.root,
        validators.iter().collect(),
        weights
    ));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 120);

    let vs1 = context
        .lst_contract
        .get_validators(Some(0), Some(6))
        .await
        .unwrap();
    let vs2 = context
        .lst_contract
        .get_validators(Some(6), Some(6))
        .await
        .unwrap();
    assert_eq!(vs1.len(), 6);
    assert_eq!(vs2.len(), 6);
}

#[tokio::test]
async fn test_vpool_whitelist() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));

    let v1: AccountId = "foo".parse().unwrap();
    let v2: AccountId = "bar".parse().unwrap();

    // set whitelist account
    check!(context.mock_whitelist.add_whitelist(&context.root, &v1));

    // try to add an validator not in whitelist
    check!(
        context.lst_contract.add_validator(&context.root, &v2, 10),
        ERR_WHITELIST
    );
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    assert_eq!(vs.len(), 0);

    // try to add an validator in whitelist
    check!(context
        .lst_contract
        .add_validators(&context.root, vec![&v1], vec![10]));
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    assert_eq!(vs.len(), 1);
    assert_eq!(vs.first().unwrap().account_id.to_string(), "foo");
}

#[tokio::test]
async fn test_vpool_remove_validator() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.mock_whitelist.allow_all(&context.root));

    // add foo, bar
    let v1: AccountId = "foo".parse().unwrap();
    let v2: AccountId = "bar".parse().unwrap();
    check!(context.lst_contract.add_validator(&context.root, &v1, 10));
    check!(context.lst_contract.add_validator(&context.root, &v2, 20));

    // remove foo
    check!(context.lst_contract.remove_validator(&context.root, &v1));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 20);
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    assert_eq!(vs.len(), 1);
    assert_eq!(vs.first().unwrap().account_id.to_string(), "bar");

    // remove bar
    check!(context.lst_contract.remove_validator(&context.root, &v2));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 0);
    let vs = context
        .lst_contract
        .get_validators(None, None)
        .await
        .unwrap();
    assert_eq!(vs.len(), 0);
}

#[tokio::test]
async fn test_vpool_update_weight() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, None).await;

    check!(context.lst_contract.acl_grant_role(
        &context.root,
        "OpManager".to_string(),
        context.manager.id()
    ));
    check!(context.mock_whitelist.allow_all(&context.root));

    // add foo, bar
    let v1: AccountId = "foo".parse().unwrap();
    let v2: AccountId = "bar".parse().unwrap();
    check!(context.lst_contract.add_validator(&context.root, &v1, 10));
    check!(context.lst_contract.add_validator(&context.root, &v2, 20));

    // update foo
    check!(context.lst_contract.update_weight(&context.root, &v1, 30));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 50);

    // update foo and bar
    check!(context
        .lst_contract
        .update_weights(&context.root, vec![&v1, &v2], vec![100, 150]));
    assert_eq!(context.lst_contract.get_total_weight().await.unwrap(), 250);
}

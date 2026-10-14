mod setup;
use setup::*;

async fn setup_fault_case_with_reserve(fund_reserve: bool) -> (Worker<Sandbox>, Account, Contract, Contract, Contract) {
    let worker = near_workspaces::sandbox().await.unwrap();
    let root = worker.root_account().unwrap();
    let lst_wasm = std::fs::read("../../target/near/lst/lst.wasm").unwrap();
    let mock_wasm = std::fs::read("../../target/near/mock_ft/mock_ft.wasm").unwrap();
    let lst = root
        .create_subaccount("lst")
        .initial_balance(NearToken::from_near(100))
        .transact()
        .await
        .unwrap()
        .unwrap()
        .deploy(&lst_wasm)
        .await
        .unwrap()
        .unwrap();
    let wnear = root
        .create_subaccount("wrap")
        .initial_balance(NearToken::from_near(100))
        .transact()
        .await
        .unwrap()
        .unwrap()
        .deploy(&mock_wasm)
        .await
        .unwrap()
        .unwrap();
    let platform = root
        .create_subaccount("platform")
        .initial_balance(NearToken::from_near(100))
        .transact()
        .await
        .unwrap()
        .unwrap()
        .deploy(&mock_wasm)
        .await
        .unwrap()
        .unwrap();
    for (contract, symbol) in [(&wnear, "wNEAR"), (&platform, "MOCK")] {
        let init = root
            .call(contract.id(), "new_default_meta")
            .args_json(json!({"owner_id": root.id(), "total_supply": "0",
                "symbol": symbol, "decimals": 24}))
            .transact()
            .await
            .unwrap();
        assert!(init.is_success(), "{init:?}");
    }
    let init_lst = root
        .call(lst.id(), "new")
        .args_json(json!({"owner_id": root.id(), "wnear_id": wnear.id()}))
        .transact()
        .await
        .unwrap();
    assert!(init_lst.is_success(), "{init_lst:?}");
    let configured_wnear: Option<AccountId> = lst.call("get_wnear_id").view().await.unwrap().json().unwrap();
    assert_eq!(configured_wnear, Some(wnear.id().clone()));
    let reserve: U128 = lst.call("get_emergency_storage_reserve").view().await.unwrap().json().unwrap();
    assert_eq!(reserve.0, 0);
    for account_id in [lst.id(), platform.id()] {
        let registration = root
            .call(wnear.id(), "storage_deposit")
            .args_json(json!({"account_id": account_id}))
            .deposit(NearToken::from_near(1))
            .transact()
            .await
            .unwrap();
        assert!(registration.is_success(), "{registration:?}");
    }
    let add_platform = root
        .call(lst.id(), "add_platform_user")
        .args_json(json!({"platform_id": platform.id(), "refund_account_id": platform.id()}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(add_platform.is_success(), "{add_platform:?}");
    let storage = root
        .call(lst.id(), "deposit_platform_user_storage")
        .args_json(json!({"platform_id": platform.id()}))
        .deposit(NearToken::from_near(1))
        .transact()
        .await
        .unwrap();
    assert!(storage.is_success(), "{storage:?}");
    if fund_reserve {
        let funding = root
            .call(lst.id(), "deposit_emergency_storage_reserve")
            .deposit(NearToken::from_near(5))
            .transact()
            .await
            .unwrap();
        assert!(funding.is_success(), "{funding:?}");
    }
    (worker, root, lst, wnear, platform)
}

async fn setup_fault_case() -> (Worker<Sandbox>, Account, Contract, Contract, Contract) {
    setup_fault_case_with_reserve(true).await
}

async fn seed_shared_wnear(platform: &Contract, wnear: &Contract, lst: &Contract) {
    let total = NearToken::from_near(20).as_yoctonear();
    let shared = NearToken::from_near(10).as_yoctonear();
    let wrap = platform
        .as_account()
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(total))
        .transact()
        .await
        .unwrap();
    assert!(wrap.is_success(), "{wrap:?}");
    let transfer = platform
        .as_account()
        .call(wnear.id(), "ft_transfer")
        .args_json(json!({"receiver_id": lst.id(), "amount": shared.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(transfer.is_success(), "{transfer:?}");
}

async fn token_balance(token: &Contract, account_id: &AccountId) -> u128 {
    let balance: U128 = token
        .call("ft_balance_of")
        .args_json(json!({"account_id": account_id}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    balance.0
}

async fn wrap_wnear(account: &Contract, wnear: &Contract, amount: u128) {
    let outcome = account
        .as_account()
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(amount))
        .transact()
        .await
        .unwrap();
    assert!(outcome.is_success(), "{outcome:?}");
}

async fn stake_wnear(
    platform: &Contract,
    wnear: &Contract,
    lst: &Contract,
    amount: u128,
    message: near_sdk::serde_json::Value,
    gas: u64,
) -> ExecutionFinalResult {
    platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(), "msg": message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(gas))
        .transact()
        .await
        .unwrap()
}

#[tokio::test]
async fn plan_a_stake_opens_only_after_reserve_reaches_five_near() {
    let (_worker, _root, lst, wnear, platform) = setup_fault_case_with_reserve(false).await;
    let amount = NearToken::from_near(1).as_yoctonear();
    wrap_wnear(&platform, &wnear, amount).await;
    let route = json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let message = json!({"receiver_id": platform.id(), "msg": "0", "refund_message": route});

    let zero_deposit = platform
        .as_account()
        .call(lst.id(), "deposit_emergency_storage_reserve")
        .transact()
        .await
        .unwrap();
    assert!(!zero_deposit.is_success());
    for expected_reserve in [0, NearToken::from_near(5).as_yoctonear() - 1] {
        let stake = stake_wnear(&platform, &wnear, &lst, amount, message.clone(), 150).await;
        assert!(stake.is_success(), "{stake:?}");
        assert!(!stake.logs().iter().any(|log| log.contains("\"event\":\"stake_accepted\"")), "{stake:?}");
        assert_eq!(token_balance(&wnear, platform.id()).await, amount);
        let reserve: U128 = lst.call("get_emergency_storage_reserve").view().await.unwrap().json().unwrap();
        assert_eq!(reserve.0, expected_reserve);
        if expected_reserve == 0 {
            let funding = platform
                .as_account()
                .call(lst.id(), "deposit_emergency_storage_reserve")
                .deposit(NearToken::from_yoctonear(NearToken::from_near(5).as_yoctonear() - 1))
                .transact()
                .await
                .unwrap();
            assert!(funding.is_success(), "{funding:?}");
            assert!(funding
                .logs()
                .iter()
                .any(|log| log.contains("\"event\":\"emergency_storage_reserve_deposited\"")));
        }
    }

    let final_funding = platform
        .as_account()
        .call(lst.id(), "deposit_emergency_storage_reserve")
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(final_funding.is_success(), "{final_funding:?}");
    let reserve: U128 = lst.call("get_emergency_storage_reserve").view().await.unwrap().json().unwrap();
    assert_eq!(reserve.0, NearToken::from_near(5).as_yoctonear());

    let stake = stake_wnear(&platform, &wnear, &lst, amount, message, 150).await;
    assert!(stake.is_success(), "{stake:?}");
    assert!(stake.logs().iter().any(|log| log.contains("\"event\":\"stake_accepted\"")), "{stake:?}");
    assert_eq!(token_balance(&wnear, platform.id()).await, 0);
    let reserve: U128 = lst.call("get_emergency_storage_reserve").view().await.unwrap().json().unwrap();
    assert_eq!(reserve.0, NearToken::from_near(5).as_yoctonear());
}

#[tokio::test]
async fn plan_a_stake_unstake_and_permissionless_withdraw() {
    let (_worker, root, lst, wnear, platform) = setup_fault_case().await;
    let registered: Option<StorageBalance> = lst
        .call("storage_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(registered.is_some());
    let forbidden_unregister = platform
        .as_account()
        .call(lst.id(), "storage_unregister")
        .args_json(json!({}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(!forbidden_unregister.is_success());

    let amount = NearToken::from_near(10).as_yoctonear();
    let wrap = platform
        .as_account()
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(amount))
        .transact()
        .await
        .unwrap();
    assert!(wrap.is_success(), "{wrap:?}");
    let unstake_message = json!({"receiver_id": platform.id(),
        "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let explicit_false = json!({"receiver_id": platform.id(),
        "withdraw_tokens": {"wnear": {"is_storage_deposit": false, "msg": "0"}}});
    let key_omitted: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": unstake_message}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let key_false: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": explicit_false}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(key_omitted, key_false);
    let invalid_message = json!({"receiver_id": platform.id(), "msg": "0",
        "is_storage_deposit": true, "refund_message": unstake_message});
    let invalid_stake = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": invalid_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(invalid_stake.is_success(), "{invalid_stake:?}");
    let refunded_wnear: U128 = wnear
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(refunded_wnear.0, amount);
    let platform_status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(platform_status.storage_reserved_bytes, 0);
    for (missing_field, message) in [
        ("msg", json!({"receiver_id": platform.id(), "refund_message": unstake_message})),
        ("refund_message", json!({"receiver_id": platform.id(), "msg": "0"})),
        (
            "refund_message.withdraw_tokens.wnear.msg",
            json!({"receiver_id": platform.id(), "msg": "0", "refund_message": {
                "receiver_id": platform.id(), "withdraw_tokens": {"wnear": {}}
            }}),
        ),
        (
            "empty refund_message.withdraw_tokens.wnear.msg",
            json!({"receiver_id": platform.id(), "msg": "0", "refund_message": {
                "receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": ""}}
            }}),
        ),
        (
            "native refund_message.withdraw_tokens",
            json!({"receiver_id": platform.id(), "msg": "0", "refund_message": {
                "receiver_id": platform.id(), "withdraw_tokens": "native"
            }}),
        ),
    ] {
        let rejected = platform
            .as_account()
            .call(wnear.id(), "ft_transfer_call")
            .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
                "msg": message.to_string()}))
            .deposit(NearToken::from_yoctonear(1))
            .gas(Gas::from_tgas(150))
            .transact()
            .await
            .unwrap();
        assert!(rejected.is_success(), "missing {missing_field}: {rejected:?}");
        assert!(!rejected.logs().iter().any(|log| log.contains("stake_accepted")));
        assert_eq!(token_balance(&wnear, platform.id()).await, amount);
        assert_eq!(token_balance(&lst, platform.id()).await, 0);
        let status: lst::PlatformUserView = lst
            .call("get_platform_user")
            .args_json(json!({"platform_id": platform.id()}))
            .view()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(status.storage_reserved_bytes, 0);
        assert_eq!(status.inflight_storage_operations, 0);
    }
    let nested_msg = json!({"receiver_id": "private-user", "msg": "opaque-payload",
        "min_gas": "5000000000000"})
    .to_string();
    let stake_message = json!({"receiver_id": platform.id(), "msg": nested_msg,
        "min_gas": "30000000000000", "refund_message": unstake_message});
    let stake = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": stake_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(stake.is_success(), "{stake:?}");
    assert!(stake.logs().iter().any(|log| log.contains("\"memo\":\"stake\"")), "{stake:?}");
    assert!(
        stake.logs().iter().any(|log| log.contains("\"event\":\"platform_stake_delivered\"")),
        "{stake:?}"
    );
    let shares: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(shares.0 > 0, "No shares delivered: {stake:?}");
    let forwarded: Option<String> = platform.call("last_received_msg").view().await.unwrap().json().unwrap();
    assert_eq!(forwarded.as_deref(), Some(nested_msg.as_str()));

    let invalid_unstake_message = json!({"receiver_id": platform.id(),
        "withdraw_tokens": {"wnear": {"is_storage_deposit": true, "msg": "0"}}});
    let invalid_unstake = platform
        .as_account()
        .call(lst.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": shares.0.to_string(),
            "msg": invalid_unstake_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(invalid_unstake.is_success(), "{invalid_unstake:?}");
    let shares_after_reject: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(shares_after_reject.0, shares.0);

    for (route_name, route) in [
        ("missing wNEAR msg", json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {}}})),
        (
            "empty wNEAR msg",
            json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": ""}}}),
        ),
        ("native route", json!({"receiver_id": platform.id(), "withdraw_tokens": "native"})),
    ] {
        let rejected = platform
            .as_account()
            .call(lst.id(), "ft_transfer_call")
            .args_json(json!({"receiver_id": lst.id(), "amount": shares.0.to_string(),
                "msg": route.to_string()}))
            .deposit(NearToken::from_yoctonear(1))
            .gas(Gas::from_tgas(200))
            .transact()
            .await
            .unwrap();
        assert!(rejected.is_success(), "{route_name}: {rejected:?}");
        assert_eq!(token_balance(&lst, platform.id()).await, shares.0);
        let requests: Vec<lst::WithdrawalRequestView> = lst
            .call("get_withdrawal_requests")
            .args_json(json!({"platform_id": platform.id()}))
            .view()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert!(requests.is_empty(), "{route_name}: {rejected:?}");
    }

    let unstake = platform
        .as_account()
        .call(lst.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": shares.0.to_string(),
            "msg": unstake_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(unstake.is_success(), "{unstake:?}");
    assert!(unstake.logs().iter().any(|log| log.contains("\"memo\":\"unstake\"")), "{unstake:?}");
    let requests: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request_key, key_omitted);
    assert_eq!(requests[0].tranches[0].amount.0, amount);
    let by_args: Option<Vec<lst::WithdrawalTrancheView>> = lst
        .call("get_withdrawal_request_tranches_by_args")
        .args_json(json!({"platform_id": platform.id(), "args": explicit_false}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(by_args.unwrap()[0].amount.0, amount);

    let advance = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": requests[0].tranches[0].available_epoch_height}))
        .transact()
        .await
        .unwrap();
    assert!(advance.is_success(), "{advance:?}");
    let request_key = json!(requests[0].request_key);
    let insufficient_gas = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": request_key}))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(!insufficient_gas.is_success(), "{insufficient_gas:?}");
    let unchanged: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": request_key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let unchanged = unchanged.unwrap();
    assert_eq!(unchanged.tranches[0].amount.0, amount);
    assert!(!unchanged.tranches[0].locked);
    let withdraw = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": request_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(withdraw.is_success(), "{withdraw:?}");
    let remaining: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(remaining.is_empty(), "Request was not settled: {withdraw:?}");
    let pending: U128 = lst.call("get_total_pending_withdrawal_amount").view().await.unwrap().json().unwrap();
    assert_eq!(pending.0, 0);
    let wnear_balance: U128 = wnear
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(wnear_balance.0, amount);

    let partial_route = json!({"receiver_id": platform.id(),
        "withdraw_tokens": {"wnear": {"msg": "half_once"}}});
    let second_stake_message = json!({"receiver_id": platform.id(), "msg": "0",
        "min_gas": "30000000000000", "refund_message": partial_route});
    let second_stake = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": second_stake_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(second_stake.is_success(), "{second_stake:?}");
    let second_shares: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let second_unstake = platform
        .as_account()
        .call(lst.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": second_shares.0.to_string(),
            "msg": partial_route.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(second_unstake.is_success(), "{second_unstake:?}");
    let second_requests: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(second_requests.len(), 1);
    let second_key = json!(second_requests[0].request_key);
    let advance_again = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": second_requests[0].tranches[0].available_epoch_height}))
        .transact()
        .await
        .unwrap();
    assert!(advance_again.is_success(), "{advance_again:?}");
    let partial = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": second_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(partial.is_success(), "{partial:?}");
    let partial_requests: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(partial_requests.len(), 1);
    assert_eq!(partial_requests[0].tranches[0].amount.0, amount / 2);
    assert_eq!(partial_requests[0].tranches[0].wnear_residual.0, amount / 2);
    assert!(!partial_requests[0].tranches[0].locked);
    let retry = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": second_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(retry.is_success(), "{retry:?}");
    let after_retry: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(after_retry.is_empty(), "{retry:?}");
    let final_wnear_balance: U128 = wnear
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(final_wnear_balance.0, amount);

    let recipient = root
        .create_subaccount("recipient")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await
        .unwrap()
        .unwrap()
        .deploy(&std::fs::read("../../target/near/mock_ft/mock_ft.wasm").unwrap())
        .await
        .unwrap()
        .unwrap();
    let init_recipient = root
        .call(recipient.id(), "new_default_meta")
        .args_json(json!({"owner_id": root.id(), "total_supply": "0",
            "symbol": "MOCK", "decimals": 24}))
        .transact()
        .await
        .unwrap();
    assert!(init_recipient.is_success(), "{init_recipient:?}");
    let unregistered_route = json!({"receiver_id": recipient.id(),
        "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let third_stake_message = json!({"receiver_id": platform.id(), "msg": "0",
        "refund_message": unregistered_route});
    let third_stake = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": third_stake_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(third_stake.is_success(), "{third_stake:?}");
    let third_shares: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let third_unstake = platform
        .as_account()
        .call(lst.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": third_shares.0.to_string(),
            "msg": unregistered_route.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(third_unstake.is_success(), "{third_unstake:?}");
    let third_requests: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(third_requests.len(), 1);
    let third_key = json!(third_requests[0].request_key);
    let advance_third = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": third_requests[0].tranches[0].available_epoch_height}))
        .transact()
        .await
        .unwrap();
    assert!(advance_third.is_success(), "{advance_third:?}");
    let failed_transfer = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": third_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(failed_transfer.is_success(), "{failed_transfer:?}");
    let retryable: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(retryable.len(), 1);
    assert!(retryable[0].tranches[0].locked);
    let recover = root
        .call(lst.id(), "recover_wnear_transfer")
        .args_json(json!({"platform_id": platform.id(), "request_key": third_key,
            "consumed": "0", "evidence": failed_transfer.outcome().transaction_hash.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(recover.is_success(), "{recover:?}");
    let recovered: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(recovered[0].tranches[0].wnear_residual.0, amount);
    assert!(!recovered[0].tranches[0].locked);
    let register_recipient = root
        .call(wnear.id(), "storage_deposit")
        .args_json(json!({"account_id": recipient.id()}))
        .deposit(NearToken::from_near(1))
        .transact()
        .await
        .unwrap();
    assert!(register_recipient.is_success(), "{register_recipient:?}");
    let successful_retry = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": third_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(successful_retry.is_success(), "{successful_retry:?}");
    let recipient_balance: U128 = wnear
        .call("ft_balance_of")
        .args_json(json!({"account_id": recipient.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(recipient_balance.0, amount);

    let replenish = platform
        .as_account()
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(amount))
        .transact()
        .await
        .unwrap();
    assert!(replenish.is_success(), "{replenish:?}");

    let failed_delivery_message = json!({"receiver_id": platform.id(), "msg": "panic",
        "min_gas": "30000000000000", "refund_message": unstake_message});
    let failed_delivery = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": failed_delivery_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(failed_delivery.is_success(), "{failed_delivery:?}");
    assert!(
        failed_delivery
            .logs()
            .iter()
            .any(|log| log.contains("\"event\":\"platform_stake_delivery_refunded\"")),
        "{failed_delivery:?}"
    );
    let refunded_shares: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(refunded_shares.0, 0);
    let automatic_requests: Vec<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_requests")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(automatic_requests.len(), 1);
    assert_eq!(automatic_requests[0].tranches[0].amount.0, amount);

    let boundary_amount = NearToken::from_near(1).as_yoctonear();
    let boundary_wrap = platform
        .as_account()
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(boundary_amount))
        .transact()
        .await
        .unwrap();
    assert!(boundary_wrap.is_success(), "{boundary_wrap:?}");
    let opaque = "x".repeat(2048);
    let boundary_message = json!({
        "receiver_id": platform.id(),
        "msg": opaque,
        "min_gas": "30000000000000",
        "refund_message": {
            "receiver_id": platform.id(),
            "withdraw_tokens": {"wnear": {"msg": opaque}}
        }
    });
    let boundary_stake = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": boundary_amount.to_string(),
            "msg": boundary_message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(boundary_stake.is_success(), "{boundary_stake:?}");
    let forwarded_boundary: Option<String> = platform.call("last_received_msg").view().await.unwrap().json().unwrap();
    assert_eq!(forwarded_boundary.as_deref(), Some(opaque.as_str()));

    let boundary_shares: U128 = lst
        .call("ft_balance_of")
        .args_json(json!({"account_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let boundary_route = json!({"receiver_id": recipient.id(),
        "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let boundary_unstake = platform
        .as_account()
        .call(lst.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": boundary_shares.0.to_string(),
            "msg": boundary_route.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(boundary_unstake.is_success(), "{boundary_unstake:?}");
    let boundary_key: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": boundary_route}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let boundary_request: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": boundary_key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let boundary_request = boundary_request.unwrap();
    assert_eq!(boundary_request.tranches[0].amount.0, boundary_amount);
    let advance_boundary = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": boundary_request.tranches[0].available_epoch_height}))
        .transact()
        .await
        .unwrap();
    assert!(advance_boundary.is_success(), "{advance_boundary:?}");
    let boundary_withdraw = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": boundary_key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(boundary_withdraw.is_success(), "{boundary_withdraw:?}");
    let after_boundary: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": boundary_key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(after_boundary.is_none());
    assert_eq!(token_balance(&wnear, recipient.id()).await, amount + boundary_amount);
}

#[tokio::test]
async fn plan_a_rejects_invalid_stakes_without_reserving_storage() {
    let (_worker, root, lst, wnear, platform) = setup_fault_case().await;
    let amount = NearToken::from_near(1).as_yoctonear();
    wrap_wnear(&platform, &wnear, amount).await;
    let route = json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let valid = json!({"receiver_id": platform.id(), "msg": "0", "refund_message": route});
    for (case, message, gas) in [
        ("wrong receiver", json!({"receiver_id": root.id(), "msg": "0", "refund_message": route}), 150),
        (
            "receiver gas below minimum",
            json!({"receiver_id": platform.id(), "msg": "0", "min_gas": "29000000000000", "refund_message": route}),
            150,
        ),
        (
            "receiver gas above maximum",
            json!({"receiver_id": platform.id(), "msg": "0", "min_gas": "41000000000000", "refund_message": route}),
            150,
        ),
        (
            "oversized message",
            json!({"receiver_id": platform.id(), "msg": "x".repeat(2049), "refund_message": route}),
            150,
        ),
        ("insufficient gas", valid.clone(), 110),
    ] {
        let outcome = stake_wnear(&platform, &wnear, &lst, amount, message, gas).await;
        assert!(outcome.is_success(), "{case}: {outcome:?}");
        assert!(
            !outcome.logs().iter().any(|log| log.contains("\"event\":\"stake_accepted\"")),
            "{case}: {outcome:?}"
        );
        assert_eq!(token_balance(&wnear, platform.id()).await, amount, "{case}");
        assert_eq!(token_balance(&lst, platform.id()).await, 0, "{case}");
        let status: lst::PlatformUserView = lst
            .call("get_platform_user")
            .args_json(json!({"platform_id": platform.id()}))
            .view()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(status.storage_reserved_bytes, 0, "{case}");
        assert_eq!(status.inflight_storage_operations, 0, "{case}");
    }
    let root_wrap = root
        .call(wnear.id(), "near_deposit")
        .deposit(NearToken::from_yoctonear(amount))
        .transact()
        .await
        .unwrap();
    assert!(root_wrap.is_success(), "{root_wrap:?}");
    let unregistered = root
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(), "msg": valid.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(unregistered.is_success(), "{unregistered:?}");
    assert!(!unregistered.logs().iter().any(|log| log.contains("\"event\":\"stake_accepted\"")));
    assert_eq!(token_balance(&wnear, root.id()).await, amount);

    let withdraw_credit = platform
        .as_account()
        .call(lst.id(), "withdraw_unused_platform_user_storage")
        .args_json(json!({"platform_id": platform.id(), "amount": NearToken::from_near(1).as_yoctonear().to_string()}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await
        .unwrap();
    assert!(withdraw_credit.is_success(), "{withdraw_credit:?}");
    let no_credit: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(no_credit.storage_deposit_total.0, 0);
    let rejected_for_credit = stake_wnear(&platform, &wnear, &lst, amount, valid.clone(), 150).await;
    assert!(rejected_for_credit.is_success(), "{rejected_for_credit:?}");
    assert_eq!(token_balance(&wnear, platform.id()).await, amount);
    let replenish = root
        .call(lst.id(), "deposit_platform_user_storage")
        .args_json(json!({"platform_id": platform.id()}))
        .deposit(NearToken::from_near(1))
        .transact()
        .await
        .unwrap();
    assert!(replenish.is_success(), "{replenish:?}");
    let drain = root
        .call(lst.id(), "set_platform_user_draining")
        .args_json(json!({"platform_id": platform.id()}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(drain.is_success(), "{drain:?}");
    let rejected = stake_wnear(&platform, &wnear, &lst, amount, valid, 150).await;
    assert!(rejected.is_success(), "{rejected:?}");
    assert_eq!(token_balance(&wnear, platform.id()).await, amount);
    assert_eq!(token_balance(&lst, platform.id()).await, 0);
}

#[tokio::test]
async fn plan_a_failed_unwrap_refunds_input_and_releases_reservation() {
    let (_worker, root, lst, wnear, platform) = setup_fault_case().await;
    let amount = NearToken::from_near(1).as_yoctonear();
    wrap_wnear(&platform, &wnear, amount).await;
    let fail = root
        .call(wnear.id(), "set_fail_next_withdraw")
        .args_json(json!({"fail": true}))
        .transact()
        .await
        .unwrap();
    assert!(fail.is_success(), "{fail:?}");
    let message = json!({"receiver_id": platform.id(), "msg": "0", "refund_message": {
        "receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": "0"}}
    }});
    let outcome = stake_wnear(&platform, &wnear, &lst, amount, message.clone(), 150).await;
    assert!(outcome.is_success(), "{outcome:?}");
    assert!(
        outcome.logs().iter().any(|log| log.contains("\"event\":\"stake_unwrap_failed\"")),
        "{outcome:?}"
    );
    assert_eq!(token_balance(&wnear, platform.id()).await, amount);
    assert_eq!(token_balance(&wnear, lst.id()).await, 0);
    assert_eq!(token_balance(&lst, platform.id()).await, 0);
    let status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status.storage_reserved_bytes, 0);
    assert_eq!(status.inflight_storage_operations, 0);

    let restore = root
        .call(wnear.id(), "set_fail_next_withdraw")
        .args_json(json!({"fail": false}))
        .transact()
        .await
        .unwrap();
    assert!(restore.is_success(), "{restore:?}");
    let retry = stake_wnear(&platform, &wnear, &lst, amount, message, 150).await;
    assert!(retry.is_success(), "{retry:?}");
    assert!(
        retry.logs().iter().any(|log| log.contains("\"event\":\"platform_stake_delivered\"")),
        "{retry:?}"
    );
}

#[tokio::test]
async fn plan_a_maximum_refund_message_fits_150_tgas_on_rejection() {
    let (_worker, _root, lst, wnear, platform) = setup_fault_case().await;
    let amount = NearToken::from_near(1).as_yoctonear();
    wrap_wnear(&platform, &wnear, amount).await;
    let route = json!({"receiver_id": platform.id(), "withdraw_tokens": {
        "wnear": {"msg": "x".repeat(2048)}
    }});
    let message = json!({"receiver_id": platform.id(), "msg": "panic", "min_gas": "30000000000000", "refund_message": route});
    let outcome = stake_wnear(&platform, &wnear, &lst, amount, message, 150).await;
    assert!(outcome.is_success(), "{outcome:?}");
    assert!(
        outcome.logs().iter().any(|log| log.contains("\"event\":\"platform_stake_delivery_refunded\"")),
        "{outcome:?}"
    );
    assert_eq!(token_balance(&lst, platform.id()).await, 0);
    let request_key: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": route}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let request: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": request_key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(request.unwrap().tranches[0].amount.0, amount);
    let status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status.storage_reserved_bytes, 0);
    assert_eq!(status.inflight_storage_operations, 0);
    assert!(status.storage_used_bytes > 0);
}

#[tokio::test]
async fn plan_a_partial_share_refund_only_enqueues_returned_shares() {
    let (_worker, _root, lst, wnear, platform) = setup_fault_case().await;
    let amount = NearToken::from_near(2).as_yoctonear();
    wrap_wnear(&platform, &wnear, amount).await;
    let route = json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let message = json!({"receiver_id": platform.id(), "msg": "half", "min_gas": "30000000000000", "refund_message": route});
    let outcome = stake_wnear(&platform, &wnear, &lst, amount, message, 150).await;
    assert!(outcome.is_success(), "{outcome:?}");
    assert!(
        outcome.logs().iter().any(|log| log.contains("\"event\":\"platform_stake_delivery_refunded\"")),
        "{outcome:?}"
    );
    assert_eq!(token_balance(&lst, platform.id()).await, amount / 2);
    let key: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": route}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let request: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(request.unwrap().tranches[0].amount.0, amount / 2);
    let pending: U128 = lst.call("get_total_pending_withdrawal_amount").view().await.unwrap().json().unwrap();
    assert_eq!(pending.0, amount / 2);
    let status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status.storage_reserved_bytes, 0);
    assert_eq!(status.inflight_storage_operations, 0);
}

#[tokio::test]
async fn plan_a_same_request_key_isolated_by_platform() {
    let (_worker, root, lst, wnear, first) = setup_fault_case().await;
    let second = root
        .create_subaccount("second-platform")
        .initial_balance(NearToken::from_near(100))
        .transact()
        .await
        .unwrap()
        .unwrap()
        .deploy(&std::fs::read("../../target/near/mock_ft/mock_ft.wasm").unwrap())
        .await
        .unwrap()
        .unwrap();
    let init = root
        .call(second.id(), "new_default_meta")
        .args_json(json!({"owner_id": root.id(), "total_supply": "0", "symbol": "MOCK", "decimals": 24}))
        .transact()
        .await
        .unwrap();
    assert!(init.is_success(), "{init:?}");
    let register = root
        .call(wnear.id(), "storage_deposit")
        .args_json(json!({"account_id": second.id()}))
        .deposit(NearToken::from_near(1))
        .transact()
        .await
        .unwrap();
    assert!(register.is_success(), "{register:?}");
    let add = root
        .call(lst.id(), "add_platform_user")
        .args_json(json!({"platform_id": second.id(), "refund_account_id": second.id()}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await
        .unwrap();
    assert!(add.is_success(), "{add:?}");
    let storage = root
        .call(lst.id(), "deposit_platform_user_storage")
        .args_json(json!({"platform_id": second.id()}))
        .deposit(NearToken::from_near(1))
        .transact()
        .await
        .unwrap();
    assert!(storage.is_success(), "{storage:?}");

    let amount = NearToken::from_near(1).as_yoctonear();
    let route = json!({"receiver_id": first.id(), "withdraw_tokens": {"wnear": {"msg": "0"}}});
    let key: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": route}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    for platform in [&first, &second] {
        wrap_wnear(platform, &wnear, amount).await;
        let message = json!({"receiver_id": platform.id(), "msg": "0", "refund_message": route});
        let stake = stake_wnear(platform, &wnear, &lst, amount, message, 150).await;
        assert!(stake.is_success(), "{stake:?}");
        let shares = token_balance(&lst, platform.id()).await;
        assert!(shares > 0);
        let unstake = platform
            .as_account()
            .call(lst.id(), "ft_transfer_call")
            .args_json(json!({"receiver_id": lst.id(), "amount": shares.to_string(), "msg": route.to_string()}))
            .deposit(NearToken::from_yoctonear(1))
            .gas(Gas::from_tgas(200))
            .transact()
            .await
            .unwrap();
        assert!(unstake.is_success(), "{unstake:?}");
        let request: Option<lst::WithdrawalRequestView> = lst
            .call("get_withdrawal_request")
            .args_json(json!({"platform_id": platform.id(), "request_key": key}))
            .view()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(request.unwrap().tranches[0].amount.0, amount);
    }
    let pending: U128 = lst.call("get_total_pending_withdrawal_amount").view().await.unwrap().json().unwrap();
    assert_eq!(pending.0, amount * 2);
    let first_request: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": first.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let mature = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": first_request.unwrap().tranches[0].available_epoch_height}))
        .transact()
        .await
        .unwrap();
    assert!(mature.is_success(), "{mature:?}");
    let withdraw = root
        .call(lst.id(), "withdraw_request")
        .args_json(json!({"platform_id": first.id(), "request_key": key}))
        .gas(Gas::from_tgas(200))
        .transact()
        .await
        .unwrap();
    assert!(withdraw.is_success(), "{withdraw:?}");
    let first_after: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": first.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let second_after: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": second.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(first_after.is_none());
    assert_eq!(second_after.unwrap().tranches[0].amount.0, amount);
    let pending_after: U128 = lst.call("get_total_pending_withdrawal_amount").view().await.unwrap().json().unwrap();
    assert_eq!(pending_after.0, amount);
}

#[tokio::test]
async fn plan_a_refund_at_tranche_limit_fits_150_tgas() {
    let (_worker, root, lst, wnear, platform) = setup_fault_case().await;
    let amount = NearToken::from_near(1).as_yoctonear();
    let route = json!({"receiver_id": platform.id(), "withdraw_tokens": {"wnear": {"msg": "x".repeat(2048)}}});
    let key: near_sdk::json_types::Base58CryptoHash = lst
        .call("get_request_key")
        .args_json(json!({"args": route}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    wrap_wnear(&platform, &wnear, amount * 16).await;

    for epoch in 10..25 {
        let advance = root
            .call(lst.id(), "set_epoch_height")
            .args_json(json!({"epoch": epoch}))
            .transact()
            .await
            .unwrap();
        assert!(advance.is_success(), "{advance:?}");
        let message = json!({"receiver_id": platform.id(), "msg": "0", "min_gas": "30000000000000", "refund_message": route});
        let stake = stake_wnear(&platform, &wnear, &lst, amount, message, 150).await;
        assert!(stake.is_success(), "epoch {epoch}: {stake:?}");
        let shares = token_balance(&lst, platform.id()).await;
        assert!(shares > 0);
        let unstake = platform
            .as_account()
            .call(lst.id(), "ft_transfer_call")
            .args_json(json!({"receiver_id": lst.id(), "amount": shares.to_string(), "msg": route.to_string()}))
            .deposit(NearToken::from_yoctonear(1))
            .gas(Gas::from_tgas(200))
            .transact()
            .await
            .unwrap();
        assert!(unstake.is_success(), "epoch {epoch}: {unstake:?}");
    }
    let before: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(before.unwrap().tranches.len(), 15);

    let advance = root
        .call(lst.id(), "set_epoch_height")
        .args_json(json!({"epoch": 25}))
        .transact()
        .await
        .unwrap();
    assert!(advance.is_success(), "{advance:?}");
    let rejection = json!({"receiver_id": platform.id(), "msg": "panic", "min_gas": "30000000000000", "refund_message": route});
    let outcome = stake_wnear(&platform, &wnear, &lst, amount, rejection, 150).await;
    assert!(outcome.is_success(), "{outcome:?}");
    assert!(
        outcome.logs().iter().any(|log| log.contains("\"event\":\"platform_stake_delivery_refunded\"")),
        "{outcome:?}"
    );
    let after: Option<lst::WithdrawalRequestView> = lst
        .call("get_withdrawal_request")
        .args_json(json!({"platform_id": platform.id(), "request_key": key}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    let after = after.unwrap();
    assert_eq!(after.tranches.len(), 16);
    assert!(after.tranches.iter().all(|tranche| tranche.amount.0 == amount && !tranche.locked));
    let status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status.storage_reserved_bytes, 0);
    assert_eq!(status.inflight_storage_operations, 0);
    let pending: U128 = lst.call("get_total_pending_withdrawal_amount").view().await.unwrap().json().unwrap();
    assert_eq!(pending.0, amount * 16);
}

#[tokio::test]
async fn plan_a_rewrap_failure_does_not_refund_from_shared_wnear() {
    let (_worker, root, lst, wnear, platform) = setup_fault_case().await;
    seed_shared_wnear(&platform, &wnear, &lst).await;
    let shared = NearToken::from_near(10).as_yoctonear();
    let amount = NearToken::from_near(5).as_yoctonear();
    let message = json!({
        "receiver_id": platform.id(), "msg": "0", "memo": "__force_rewrap__",
        "refund_message": {"receiver_id": platform.id(),
            "withdraw_tokens": {"wnear": {"msg": "0"}}}
    });
    let success = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(success.is_success(), "{success:?}");
    assert_eq!(token_balance(&wnear, platform.id()).await, shared);
    assert_eq!(token_balance(&wnear, lst.id()).await, shared);

    let fail_wrap = root
        .call(wnear.id(), "set_fail_next_deposit")
        .args_json(json!({"fail": true}))
        .transact()
        .await
        .unwrap();
    assert!(fail_wrap.is_success(), "{fail_wrap:?}");
    let failed = platform
        .as_account()
        .call(wnear.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
            "msg": message.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(150))
        .transact()
        .await
        .unwrap();
    assert!(failed.is_success(), "{failed:?}");
    assert!(failed.logs().iter().any(|log| log.contains("\"event\":\"stake_rewrap_failed\"")));
    assert_eq!(token_balance(&wnear, platform.id()).await, amount);
    assert_eq!(token_balance(&wnear, lst.id()).await, shared);
    let status: lst::PlatformUserView = lst
        .call("get_platform_user")
        .args_json(json!({"platform_id": platform.id()}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(status.storage_reserved_bytes, 0);
    assert_eq!(status.inflight_storage_operations, 0);
}

#[tokio::test]
async fn plan_a_callback_failure_exposes_shared_balance_risk() {
    let (_worker, _root, lst, wnear, platform) = setup_fault_case().await;
    seed_shared_wnear(&platform, &wnear, &lst).await;
    let shared = NearToken::from_near(10).as_yoctonear();
    let amount = NearToken::from_near(5).as_yoctonear();
    for (memo, expected_inflight) in [("__panic_after_unwrap__", 1), ("__callback_oog__", 2)] {
        let message = json!({
            "receiver_id": platform.id(), "msg": "0", "memo": memo,
            "refund_message": {"receiver_id": platform.id(),
                "withdraw_tokens": {"wnear": {"msg": "0"}}}
        });
        let outcome = platform
            .as_account()
            .call(wnear.id(), "ft_transfer_call")
            .args_json(json!({"receiver_id": lst.id(), "amount": amount.to_string(),
                "msg": message.to_string()}))
            .deposit(NearToken::from_yoctonear(1))
            .gas(Gas::from_tgas(150))
            .transact()
            .await
            .unwrap();
        assert!(outcome.is_success(), "{outcome:?}");
        if memo == "__callback_oog__" {
            assert!(
                outcome
                    .logs()
                    .iter()
                    .any(|line| line.contains("\"event\":\"injected_callback_gas\"") && line.contains("\"gas\":1")),
                "{outcome:?}"
            );
        }
        assert_eq!(token_balance(&wnear, platform.id()).await, shared);
        assert_eq!(token_balance(&wnear, lst.id()).await, shared - amount * expected_inflight as u128);
        let status: lst::PlatformUserView = lst
            .call("get_platform_user")
            .args_json(json!({"platform_id": platform.id()}))
            .view()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(status.inflight_storage_operations, expected_inflight);
        assert!(status.storage_reserved_bytes > 0);
    }
    assert_eq!(token_balance(&wnear, lst.id()).await, 0);
}

#[tokio::test]
async fn upgrades_current_state_without_changing_supply() {
    let worker = near_workspaces::sandbox().await.unwrap();
    let context = Context::new(&worker, Some("../../target/near/lst/lst.wasm")).await;
    let supply_before = context.lst_contract.ft_total_supply().await.unwrap();
    let total_before = context.lst_contract.get_total_staked_balance().await.unwrap();
    let stage = context
        .lst_contract
        .up_stage_code(&context.root, "../../target/near/lst/lst.wasm")
        .await
        .unwrap();
    assert!(stage.is_success(), "{stage:?}");
    let hash = context.lst_contract.up_staged_code_hash().await.unwrap().unwrap();
    let deploy = context.lst_contract.up_deploy_code(&context.root, hash).await.unwrap();
    assert!(deploy.is_success(), "{deploy:?}");
    assert_eq!(context.lst_contract.ft_total_supply().await.unwrap(), supply_before);
    assert_eq!(context.lst_contract.get_total_staked_balance().await.unwrap(), total_before);
    let users: Vec<lst::PlatformUserView> = context
        .lst_contract
        .0
        .call("get_platform_users")
        .args_json(json!({}))
        .view()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(users.is_empty());
}

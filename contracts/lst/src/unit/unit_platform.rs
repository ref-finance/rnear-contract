use crate::platform::{MAX_INFLIGHT_PER_PLATFORM, MAX_MESSAGE_BYTES, MAX_TRANCHES_PER_REQUEST};
use crate::*;
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{serde_json, test_vm_config, testing_env, PromiseResult, RuntimeFeesConfig};
use std::collections::HashMap;

fn setup() -> (Contract, VMContextBuilder, AccountId) {
    let owner: AccountId = "owner.test.near".parse().unwrap();
    let platform: AccountId = "platform.test.near".parse().unwrap();
    let mut context = VMContextBuilder::new();
    context.predecessor_account_id(owner.clone());
    testing_env!(context.build());
    let mut contract = Contract::new(owner, None, "wrap.near".parse().unwrap());
    context.attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    contract.add_platform_user(platform.clone(), platform.clone());
    (contract, context, platform)
}

#[test]
fn pending_unwrap_reserve_clamps_spendable_near() {
    let (mut contract, _context, _platform) = setup();
    let available = contract.available_near();
    contract.data_mut().stake_lostfound_near_reserved = available / 2;
    assert_eq!(contract.available_near(), available - available / 2);
    contract.data_mut().stake_lostfound_near_reserved = available + 1;
    assert_eq!(contract.available_near(), 0);
}

#[test]
fn owner_recovery_wraps_and_delivers_without_withdrawal_request() {
    let (mut contract, mut context, platform) = setup();
    context.attached_deposit(NearToken::from_near(1));
    testing_env!(context.build());
    contract.deposit_platform_user_storage(platform.clone());
    let refund_message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let reserved_bytes = Contract::checked_reservation(&refund_message);
    contract.reserve_platform_storage(&platform, reserved_bytes);
    contract.save_stake_lostfound(
        0,
        Some(StakeLostFound {
            platform_id: platform.clone(),
            amount: ONE_NEAR,
            refund_message: refund_message.clone(),
            reserved_bytes,
            created_block_height: 0,
            status: StakeLostFoundStatus::Near,
        }),
        &platform,
    );
    contract.data_mut().stake_lostfound_near_reserved = ONE_NEAR;
    context.attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    contract.recover_stake_lostfound(0, StakeRecoveryAsset::Near, [0; 32].into());
    assert_eq!(contract.get_stake_lostfound(0).unwrap().status, StakeLostFoundStatus::Wrapping);
    assert_eq!(contract.data().stake_lostfound_near_reserved, ONE_NEAR);
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: Contract::request_key(&refund_message),
    };
    assert!(contract.data().withdrawal_requests.get(&key).is_none());

    context.predecessor_account_id(env::current_account_id());
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Successful(Vec::new())]
    );
    contract.on_stake_recovery_wrapped(0);
    assert_eq!(contract.get_stake_lostfound(0).unwrap().status, StakeLostFoundStatus::Sending);
    assert_eq!(contract.data().stake_lostfound_near_reserved, 0);
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Successful(serde_json::to_vec(&U128(ONE_NEAR)).unwrap())]
    );
    contract.on_stake_recovery_transfer(0);
    assert!(contract.get_stake_lostfound(0).is_none());
    assert_eq!(contract.platform_user(&platform).inflight_storage_operations, 0);
}

#[test]
fn unclear_recovery_send_needs_owner_reconciliation() {
    let (mut contract, mut context, platform) = setup();
    context.attached_deposit(NearToken::from_near(1));
    testing_env!(context.build());
    contract.deposit_platform_user_storage(platform.clone());
    let refund_message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let reserved_bytes = Contract::checked_reservation(&refund_message);
    contract.reserve_platform_storage(&platform, reserved_bytes);
    contract.save_stake_lostfound(
        0,
        Some(StakeLostFound {
            platform_id: platform.clone(),
            amount: ONE_NEAR,
            refund_message,
            reserved_bytes,
            created_block_height: 0,
            status: StakeLostFoundStatus::Wnear,
        }),
        &platform,
    );
    context.attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    contract.recover_stake_lostfound(0, StakeRecoveryAsset::Wnear, [0; 32].into());
    context.predecessor_account_id(env::current_account_id());
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Failed]
    );
    contract.on_stake_recovery_transfer(0);
    assert_eq!(contract.get_stake_lostfound(0).unwrap().status, StakeLostFoundStatus::Sending);
    context.predecessor_account_id("owner.test.near".parse().unwrap());
    context.attached_deposit(NearToken::from_yoctonear(1));
    testing_env!(context.build());
    contract.reconcile_stake_transfer(0, U128(ONE_NEAR / 2), [0; 32].into());
    let remaining = contract.get_stake_lostfound(0).unwrap();
    assert_eq!(remaining.status, StakeLostFoundStatus::Wnear);
    assert_eq!(remaining.amount.0, ONE_NEAR - ONE_NEAR / 2);
    assert_eq!(contract.platform_user(&platform).inflight_storage_operations, 1);
}

#[test]
fn unwrap_callback_prices_at_current_rate() {
    let (mut contract, mut context, platform) = setup();
    let amount = 10 * ONE_NEAR;
    contract.data_mut().total_staked_near_amount += amount;
    context.predecessor_account_id(env::current_account_id());
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Successful(Vec::new())]
    );
    let message = StakeMessage {
        receiver_id: platform.clone(),
        msg: "0".to_string(),
        memo: None,
        is_storage_deposit: false,
        min_gas: None,
        refund_message: UnstakeMessage {
            receiver_id: platform.clone(),
            withdraw_tokens: WithdrawTokens::Wnear {
                is_storage_deposit: false,
                msg: "0".to_string(),
                memo: None,
                min_gas: None,
            },
        },
    };
    contract.save_stake_lostfound(
        0,
        Some(StakeLostFound {
            platform_id: platform.clone(),
            amount,
            refund_message: message.refund_message.clone(),
            reserved_bytes: 0,
            created_block_height: 0,
            status: StakeLostFoundStatus::Processing,
        }),
        &platform,
    );
    contract.data_mut().stake_lostfound_near_reserved = amount;
    contract.on_wnear_unwrapped(0, message);
    assert!(contract.get_stake_lostfound(0).is_none());
    assert_eq!(contract.data().token.ft_balance_of(platform).0, 5 * ONE_NEAR);
    assert_eq!(contract.data().total_staked_near_amount, 30 * ONE_NEAR);
}

#[test]
fn unclear_transfer_result_keeps_request_locked() {
    let (mut contract, mut context, platform) = setup();
    let message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "opaque".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let key_hash = Contract::request_key(&message);
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: key_hash,
    };
    contract.save_request(
        &key,
        Some(WithdrawalRequest {
            message,
            tranches: vec![TrancheLock::WnearSending {
                tranche: WithdrawalTranche {
                    amount: ONE_NEAR,
                    available_epoch_height: 0,
                    wnear_residual: ONE_NEAR,
                },
                amount_to_send: ONE_NEAR,
            }],
        }),
    );
    context.predecessor_account_id(env::current_account_id());
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Failed]
    );
    contract.on_wnear_transfer(platform, key_hash.into());
    assert!(contract.load_request(&key).tranches[0].is_locked());
}

#[test]
fn sponsored_registration_is_reclaimed_on_platform_removal() {
    let (mut contract, _context, platform) = setup();
    assert!(contract.platform_user(&platform).sponsored_ft_registration);
    contract.set_platform_user_draining(platform.clone());
    contract.remove_platform_user(platform.clone());
    assert!(contract.data().token.accounts.get(&platform).is_none());
    assert!(contract.data().accounts.get(&platform).is_none());
}

#[test]
fn reserved_refund_can_fill_the_sixteenth_tranche() {
    let (mut contract, mut context, platform) = setup();
    context.attached_deposit(NearToken::from_near(1));
    testing_env!(context.build());
    contract.deposit_platform_user_storage(platform.clone());
    let message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "x".repeat(MAX_MESSAGE_BYTES),
            memo: None,
            min_gas: None,
        },
    };
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: Contract::request_key(&message),
    };
    contract.save_request(
        &key,
        Some(WithdrawalRequest {
            message: message.clone(),
            tranches: (0..MAX_TRANCHES_PER_REQUEST - 1)
                .map(|index| {
                    TrancheLock::Unlocked(WithdrawalTranche {
                        amount: ONE_NEAR,
                        available_epoch_height: 100 + index as u64,
                        wnear_residual: 0,
                    })
                })
                .collect(),
        }),
    );
    contract.data_mut().total_pending_withdrawal_amount = ONE_NEAR * (MAX_TRANCHES_PER_REQUEST as u128 - 1);
    let reserved = Contract::checked_reservation(&message);
    contract.reserve_platform_storage(&platform, reserved);
    contract.internal_unstake_shares_to_request(&platform, ONE_NEAR, &message, Some(reserved));

    let request = contract.load_request(&key);
    assert_eq!(request.tranches.len(), MAX_TRANCHES_PER_REQUEST);
    assert_eq!(contract.platform_user(&platform).storage_reserved_bytes, 0);
    assert_eq!(contract.platform_user(&platform).inflight_storage_operations, 0);
    assert_eq!(contract.data().total_inflight_storage_operations, 0);
    assert!(contract.platform_user(&platform).storage_used_bytes > 0);
}

#[test]
fn platform_inflight_limit_preserves_existing_reservations() {
    let (mut contract, mut context, platform) = setup();
    context.attached_deposit(NearToken::from_near(1));
    testing_env!(context.build());
    contract.deposit_platform_user_storage(platform.clone());
    let bytes = 512;
    for _ in 0..MAX_INFLIGHT_PER_PLATFORM {
        contract.reserve_platform_storage(&platform, bytes);
    }
    let denied = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| contract.reserve_platform_storage(&platform, bytes)));
    assert!(denied.is_err());
    assert_eq!(contract.platform_user(&platform).inflight_storage_operations, MAX_INFLIGHT_PER_PLATFORM);
    assert_eq!(
        contract.platform_user(&platform).storage_reserved_bytes,
        bytes * MAX_INFLIGHT_PER_PLATFORM as u64
    );
    assert_eq!(contract.data().total_inflight_storage_operations, MAX_INFLIGHT_PER_PLATFORM);

    contract.release_platform_storage(&platform, bytes);
    contract.reserve_platform_storage(&platform, bytes);
    assert_eq!(contract.platform_user(&platform).inflight_storage_operations, MAX_INFLIGHT_PER_PLATFORM);
}

#[test]
fn failed_wrap_unlocks_withdrawal_without_recording_wnear() {
    let (mut contract, mut context, platform) = setup();
    let message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: Contract::request_key(&message),
    };
    contract.save_request(
        &key,
        Some(WithdrawalRequest {
            message,
            tranches: vec![TrancheLock::Wrapping {
                tranche: WithdrawalTranche {
                    amount: ONE_NEAR,
                    available_epoch_height: 0,
                    wnear_residual: 0,
                },
                amount_to_wrap: ONE_NEAR,
            }],
        }),
    );
    context.predecessor_account_id(env::current_account_id());
    testing_env!(
        context.build(),
        test_vm_config(),
        RuntimeFeesConfig::test(),
        HashMap::new(),
        vec![PromiseResult::Failed]
    );
    let result = contract.on_near_deposit(platform, key.request_key.into());
    assert!(matches!(result, PromiseOrValue::Value(())));
    let request = contract.load_request(&key);
    assert!(!request.tranches[0].is_locked());
    assert_eq!(request.tranches[0].unlocked().unwrap().wnear_residual, 0);
}

#[test]
fn wrapping_recovery_records_confirmed_wnear_once() {
    let (mut contract, _context, platform) = setup();
    let message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: Contract::request_key(&message),
    };
    contract.save_request(
        &key,
        Some(WithdrawalRequest {
            message,
            tranches: vec![TrancheLock::Wrapping {
                tranche: WithdrawalTranche {
                    amount: ONE_NEAR,
                    available_epoch_height: 0,
                    wnear_residual: 0,
                },
                amount_to_wrap: ONE_NEAR,
            }],
        }),
    );
    let evidence = key.request_key.into();
    contract.recover_wrapping(platform.clone(), key.request_key.into(), true, evidence);
    let request = contract.load_request(&key);
    assert!(!request.tranches[0].is_locked());
    assert_eq!(request.tranches[0].unlocked().unwrap().wnear_residual, ONE_NEAR);
    let repeated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        contract.recover_wrapping(platform.clone(), key.request_key.into(), true, evidence);
    }));
    assert!(repeated.is_err());
    assert_eq!(contract.load_request(&key).tranches[0].unlocked().unwrap().wnear_residual, ONE_NEAR);
}

#[test]
fn unstake_merges_same_epoch_without_resetting_older_tranche() {
    let (mut contract, mut context, platform) = setup();
    context.attached_deposit(NearToken::from_near(1));
    testing_env!(context.build());
    contract.deposit_platform_user_storage(platform.clone());
    let message = UnstakeMessage {
        receiver_id: platform.clone(),
        withdraw_tokens: WithdrawTokens::Wnear {
            is_storage_deposit: false,
            msg: "0".to_string(),
            memo: None,
            min_gas: None,
        },
    };
    let key = PlatformRequestKey {
        platform_id: platform.clone(),
        request_key: Contract::request_key(&message),
    };
    contract.internal_unstake_shares_to_request(&platform, ONE_NEAR, &message, None);
    contract.internal_unstake_shares_to_request(&platform, ONE_NEAR, &message, None);
    let merged = contract.load_request(&key);
    assert_eq!(merged.tranches.len(), 1);
    assert_eq!(merged.tranches[0].unlocked().unwrap().amount, 2 * ONE_NEAR);
    let original_height = merged.tranches[0].unlocked().unwrap().available_epoch_height;

    #[cfg(feature = "test")]
    contract.set_epoch_height(11);
    #[cfg(not(feature = "test"))]
    {
        context.epoch_height(11);
        testing_env!(context.build());
    }
    contract.internal_unstake_shares_to_request(&platform, ONE_NEAR, &message, None);
    let request = contract.load_request(&key);
    assert_eq!(request.tranches.len(), 2);
    assert_eq!(request.tranches[0].unlocked().unwrap().amount, 2 * ONE_NEAR);
    assert_eq!(request.tranches[0].unlocked().unwrap().available_epoch_height, original_height);
    assert_eq!(request.tranches[1].unlocked().unwrap().amount, ONE_NEAR);
    assert!(request.tranches[1].unlocked().unwrap().available_epoch_height > original_height);
    assert_eq!(contract.data().total_pending_withdrawal_amount, 3 * ONE_NEAR);
    assert_eq!(contract.platform_user(&platform).active_request_keys, 1);
}

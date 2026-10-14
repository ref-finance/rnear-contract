use crate::*;
use near_sdk::test_utils;
use near_sdk::{
    json_types::Base58CryptoHash,
    serde_json::{self, json},
};

#[test]
fn epoch_stake_attempt() {
    let validator_id = &alice_id();
    let amount = &U128(100);
    Event::EpochStakeAttempt { validator_id, amount }.emit();
    assert_eq!(
        test_utils::get_logs()[0],
        r#"EVENT_JSON:{"data":[{"amount":"100","validator_id":"alice_id"}],"event":"epoch_stake_attempt","standard":"rhea_lst","version":"1.0.0"}"#
    );
}

#[test]
fn platform_withdrawal_failed_optional_result() {
    let platform_id: AccountId = "platform.test.near".parse().unwrap();
    let request_key = Base58CryptoHash::from([0; 32]);
    Event::WithdrawalFailed {
        platform_id: &platform_id,
        request_key,
        stage: "wrapping",
        result: None,
    }
    .emit();
    let logs = test_utils::get_logs();
    let event: serde_json::Value = serde_json::from_str(logs[0].strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    assert_eq!(event["event"], "withdrawal_failed");
    assert_eq!(
        event["data"][0],
        json!({"platform_id": platform_id, "request_key": request_key, "stage": "wrapping"})
    );

    Event::WithdrawalFailed {
        platform_id: &platform_id,
        request_key,
        stage: "wnear_sending",
        result: Some("unclear"),
    }
    .emit();
    let logs = test_utils::get_logs();
    let event: serde_json::Value = serde_json::from_str(logs[1].strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    assert_eq!(
        event["data"][0],
        json!({"platform_id": platform_id, "request_key": request_key, "stage": "wnear_sending", "result": "unclear"})
    );
}

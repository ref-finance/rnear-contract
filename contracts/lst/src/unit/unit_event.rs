use crate::*;
use near_sdk::test_utils;

#[test]
fn epoch_stake_attempt() {
    let validator_id = &alice_id();
    let amount = &U128(100);
    Event::EpochStakeAttempt {
        validator_id,
        amount,
    }
    .emit();
    assert_eq!(
        test_utils::get_logs()[0],
        r#"EVENT_JSON:{"data":[{"amount":"100","validator_id":"alice_id"}],"event":"epoch_stake_attempt","standard":"rhea_lst","version":"1.0.0"}"#
    );
}

#[test]
fn epoch_update_rewards_failed() {
    Event::EpochUpdateRewardsFailed {
        validator_id: &alice_id(),
        reason: "validator_call_failed",
    }
    .emit();
    assert_eq!(
        test_utils::get_logs()[0],
        r#"EVENT_JSON:{"data":[{"reason":"validator_call_failed","validator_id":"alice_id"}],"event":"epoch_update_rewards_failed","standard":"rhea_lst","version":"1.0.0"}"#
    );
}

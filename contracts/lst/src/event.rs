use near_sdk::{
    json_types::{Base58CryptoHash, U128},
    log,
    serde::Serialize,
    serde_json::json,
    AccountId,
};

const EVENT_STANDARD: &str = "rhea_lst";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum Event<'a> {
    // Epoch Actions
    EpochStakeAttempt {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochStakeSuccess {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochStakeFailed {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochUnstakeAttempt {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochUnstakeSuccess {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochUnstakeFailed {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochWithdrawAttempt {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochWithdrawSuccess {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochWithdrawFailed {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    EpochUpdateRewards {
        validator_id: &'a AccountId,
        old_balance: &'a U128,
        new_balance: &'a U128,
        rewards: &'a U128,
    },
    EpochCleanup {
        stake_amount_to_settle: &'a U128,
        unstake_amount_to_settle: &'a U128,
    },
    // Drain Operations
    DrainUnstakeAttempt {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    DrainUnstakeSuccess {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    DrainUnstakeFailed {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    DrainWithdrawAttempt {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    DrainWithdrawSuccess {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    DrainWithdrawFailed {
        validator_id: &'a AccountId,
        amount: &'a U128,
    },
    // Sync validator balance
    SyncValidatorBalanceSuccess {
        validator_id: &'a AccountId,
        old_staked_balance: &'a U128,
        old_unstaked_balance: &'a U128,
        old_total_balance: &'a U128,
        new_staked_balance: &'a U128,
        new_unstaked_balance: &'a U128,
        new_total_balance: &'a U128,
    },
    SyncValidatorBalanceFailedLargeDiff {
        validator_id: &'a AccountId,
        old_staked_balance: &'a U128,
        old_unstaked_balance: &'a U128,
        old_total_balance: &'a U128,
        new_staked_balance: &'a U128,
        new_unstaked_balance: &'a U128,
        new_total_balance: &'a U128,
    },
    SyncValidatorBalanceFailedCannotGetAccount {
        validator_id: &'a AccountId,
        old_staked_balance: &'a U128,
        old_unstaked_balance: &'a U128,
        old_total_balance: &'a U128,
    },
    // Staking Pool Interface
    Deposit {
        account_id: &'a AccountId,
        amount: &'a U128,
        new_unstaked_balance: &'a U128,
    },
    Withdraw {
        account_id: &'a AccountId,
        amount: &'a U128,
        new_unstaked_balance: &'a U128,
    },
    Stake {
        account_id: &'a AccountId,
        staked_amount: &'a U128,
        minted_stake_shares: &'a U128,
        new_unstaked_balance: &'a U128,
        new_stake_shares: &'a U128,
    },
    Unstake {
        account_id: &'a AccountId,
        unstaked_amount: &'a U128,
        burnt_stake_shares: &'a U128,
        new_unstaked_balance: &'a U128,
        new_stake_shares: &'a U128,
        unstaked_available_epoch_height: u64,
    },
    // Validators
    ValidatorAdded {
        account_id: &'a AccountId,
        weight: u16,
    },
    ValidatorsUpdatedWeights {
        account_ids: Vec<&'a AccountId>,
        old_weights: Vec<u16>,
        new_weights: Vec<u16>,
    },
    ValidatorUpdatedBaseStakeAmount {
        account_id: &'a AccountId,
        old_base_stake_amount: &'a U128,
        new_base_stake_amount: &'a U128,
    },
    ValidatorRemoved {
        account_id: &'a AccountId,
    },
    // Owner
    ChangeOwner {
        old_owner_id: &'a AccountId,
        new_owner_id: &'a AccountId,
    },
    AddManager {
        manager_id: &'a AccountId,
    },
    RemoveManager {
        manager_id: &'a AccountId,
    },
    SetBeneficiary {
        account_id: &'a AccountId,
        bps: &'a u32,
    },
    RemoveBeneficiary {
        account_id: &'a AccountId,
    },
    SetWhitelist {
        account_id: &'a AccountId,
    },
    PauseContract {},
    ResumeContract {},
    Donate {
        account_id: AccountId,
        amount: U128,
    },
    // Platform integration
    EmergencyStorageReserveDeposited {
        account_id: &'a AccountId,
        amount: U128,
        total: U128,
    },
    WnearConfigured {
        wnear_id: &'a AccountId,
    },
    PlatformUserStorageOverdrawn {
        platform_id: &'a AccountId,
        amount: U128,
    },
    WithdrawalRequestCreated {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        shares: U128,
        amount: U128,
        receiver_id: &'a AccountId,
        available_epoch_height: u64,
    },
    WithdrawalDelivered {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        amount: U128,
        remaining_amount: U128,
    },
    WithdrawalPartiallyRefunded {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        remaining_amount: U128,
    },
    StakeAccepted {
        platform_id: &'a AccountId,
        receiver_id: &'a AccountId,
        deposit_amount: U128,
        stake_amount: U128,
    },
    WithdrawalAttempt {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        amount: U128,
        receiver_id: &'a AccountId,
    },
    WithdrawalRecovered {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        evidence: Base58CryptoHash,
        #[serde(skip_serializing_if = "Option::is_none")]
        wrapped: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        consumed: Option<U128>,
    },
    WithdrawalFailed {
        platform_id: &'a AccountId,
        request_key: Base58CryptoHash,
        stage: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<&'a str>,
    },
    StakeUnwrapFailed {
        platform_id: &'a AccountId,
        amount: U128,
    },
    StakeCompleted {
        platform_id: &'a AccountId,
        receiver_id: &'a AccountId,
        stake_amount: U128,
        shares: U128,
    },
    StakeRewrappedForRefund {
        platform_id: &'a AccountId,
        amount: U128,
    },
    StakeRewrapFailed {
        platform_id: &'a AccountId,
        amount: U128,
    },
    PlatformStakeDeliveryRefunded {
        platform_id: &'a AccountId,
        receiver_id: &'a AccountId,
        refunded_shares: U128,
    },
    PlatformStakeDelivered {
        platform_id: &'a AccountId,
        receiver_id: &'a AccountId,
        shares: U128,
    },
    PlatformUserAdded {
        platform_id: &'a AccountId,
    },
    PlatformUserDraining {
        platform_id: &'a AccountId,
    },
    PlatformUserRemoved {
        platform_id: &'a AccountId,
    },
    PlatformUserStorageDeposited {
        platform_id: &'a AccountId,
        amount: U128,
    },
    PlatformUserStorageWithdrawn {
        platform_id: &'a AccountId,
        amount: U128,
    },
    #[cfg(feature = "test")]
    InjectedCallbackGas {
        gas: u64,
    },
}

impl Event<'_> {
    pub fn emit(&self) {
        emit_event(&self);
    }
}

pub(crate) fn emit_event<T: ?Sized + Serialize>(data: &T) {
    let result = json!(data);
    let event_json = json!({
        "standard": EVENT_STANDARD,
        "version": EVENT_STANDARD_VERSION,
        "event": result["event"],
        "data": [result["data"]]
    })
    .to_string();
    log!("EVENT_JSON:{}", event_json);
}

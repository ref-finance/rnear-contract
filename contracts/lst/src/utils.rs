use crate::*;
use uint::construct_uint;

/// No deposit balance.
pub const NO_DEPOSIT: u128 = 0;
/// one NEAR
pub const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;
pub const INIT_STAKING_AMOUNT: u128 = 10 * ONE_NEAR;
pub const INIT_STORAGE_OCCUPY: u128 = 10 * ONE_NEAR;
/// The number of epochs required for the locked balance to become unlocked.
/// NOTE: The actual number of epochs when the funds are unlocked is 3. But there is a corner case
/// when the unstaking promise can arrive at the next epoch, while the inner state is already
/// updated in the previous epoch. It will not unlock the funds for 4 epochs.
pub const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 4;
/// Full basis points, i.e. 10,000
pub const FULL_BASIS_POINTS: u32 = 10_000;

pub const MAX_BENEFICIARIES: u32 = 10;

/// min NEAR balance this contract should hold in order to cover storage
pub const CONTRACT_MIN_RESERVE_BALANCE: NearToken = NearToken::from_yoctonear(ONE_NEAR);

pub const GAS_EPOCH_STAKE: Gas = Gas::from_tgas(75);
pub const GAS_EPOCH_UNSTAKE: Gas = Gas::from_tgas(75);
pub const GAS_EPOCH_UPDATE_REWARDS: Gas = Gas::from_tgas(75);
pub const GAS_EPOCH_WITHDRAW: Gas = Gas::from_tgas(75);
pub const GAS_SYNC_BALANCE: Gas = Gas::from_tgas(75);
pub const GAS_DRAIN_UNSTAKE: Gas = Gas::from_tgas(75);
pub const GAS_DRAIN_WITHDRAW: Gas = Gas::from_tgas(75);
pub const GAS_EXT_DEPOSIT_AND_STAKE: Gas = Gas::from_tgas(75);
pub const GAS_EXT_UNSTAKE: Gas = Gas::from_tgas(75);
pub const GAS_EXT_GET_BALANCE: Gas = Gas::from_tgas(25);
pub const GAS_EXT_GET_ACCOUNT: Gas = Gas::from_tgas(25);
pub const GAS_EXT_WITHDRAW: Gas = Gas::from_tgas(75);
pub const GAS_EXT_WHITELIST: Gas = Gas::from_tgas(10);
pub const GAS_CB_VALIDATOR_SYNC_BALANCE: Gas = Gas::from_tgas(25);
pub const GAS_CB_VALIDATOR_STAKED: Gas = Gas::from_tgas(25);
pub const GAS_CB_VALIDATOR_UNSTAKED: Gas = Gas::from_tgas(25);
pub const GAS_CB_VALIDATOR_GET_BALANCE: Gas = Gas::from_tgas(25);
pub const GAS_CB_VALIDATOR_WITHDRAW: Gas = Gas::from_tgas(25);
pub const GAS_CB_WHITELIST: Gas = Gas::from_tgas(15);

construct_uint! {
    /// 256-bit unsigned integer.
    #[near(serializers = [borsh, json])]
    pub struct U256(4);
}

pub fn min3(x: u128, y: u128, z: u128) -> u128 {
    min(x, min(y, z))
}

/// The absolute diff between left and right is not greater than epsilon.
/// This is useful when user submit requests that approximately equal to the acount's NEAR/LST balance
pub(crate) fn abs_diff_eq(left: u128, right: u128, epsilon: u128) -> bool {
    left <= right + epsilon && right <= left + epsilon
}

pub(crate) fn bps_mul(value: u128, points: u32) -> u128 {
    value * (points as u128) / FULL_BASIS_POINTS as u128
}

/// According to official staking-pool contract code
/// https://github.com/near/core-contracts/blob/a4c0bf31ac4a5468c1e1839c661b26678ed8b62a/staking-pool/src/lib.rs#L127
#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct RewardFeeFraction {
    pub numerator: u32,
    pub denominator: u32,
}

#[cfg(not(feature = "test"))]
pub fn get_epoch_height() -> EpochHeight {
    env::epoch_height()
}

#[cfg(feature = "test")]
pub fn get_epoch_height() -> EpochHeight {
    let test_epoch_height_key: &[u8] = "_test_epoch_".as_bytes();
    let raw_epoch_option = env::storage_read(test_epoch_height_key);

    // default epoch is 10 for testing
    if let Some(raw_epoch) = raw_epoch_option {
        EpochHeight::try_from_slice(&raw_epoch).unwrap_or(10)
    } else {
        10
    }
}

/// for integration tests
#[near]
impl Contract {
    /// Set epoch height helper method, only available for testing
    #[cfg(feature = "test")]
    pub fn set_epoch_height(&mut self, epoch: EpochHeight) {
        let test_epoch_height_key: &[u8] = "_test_epoch_".as_bytes();
        env::storage_write(test_epoch_height_key, &epoch.to_le_bytes());
    }

    /// Read epoch height helper method, only available for testing
    #[cfg(feature = "test")]
    pub fn read_epoch_height(&self) -> EpochHeight {
        get_epoch_height()
    }

    /// Add epoch rewards method, only available for testing
    #[cfg(feature = "test")]
    pub fn add_epoch_rewards(&mut self, amount: U128) {
        let amount: u128 = amount.into();
        require!(amount > 0, "Added rewards amount must be positive");
        self.data_mut().total_staked_near_amount += amount;
    }
}

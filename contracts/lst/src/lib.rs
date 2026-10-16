use near_contract_standards::fungible_token::{
    events::{FtBurn, FtMint},
    metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC},
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds, StorageManagement};
use near_plugins::{access_control, access_control_any, pause, AccessControlRole, AccessControllable, Pausable, Upgradable};
use near_sdk::{
    assert_one_yocto,
    borsh::{BorshDeserialize, BorshSerialize},
    env, ext_contract, is_promise_success,
    json_types::{Base58CryptoHash, U128, U64},
    log, near, require,
    serde::{Deserialize, Serialize},
    store::{IterableMap, LazyOption},
    AccountId, BorshStorageKey, CryptoHash, EpochHeight, Gas, NearToken, PanicOnDefault, Promise, PromiseError, PromiseOrValue, PublicKey, StorageUsage,
};
use std::cmp::min;
use std::collections::HashMap;

mod account;
mod epoch_actions;
mod errors;
mod event;
mod ft;
mod internal;
mod owner;
mod platform;
mod stake_pool_itf;
mod storage;
#[cfg(test)]
mod unit;
mod upgrade;
mod utils;
mod validator;
mod validator_pool;
mod view;

pub use account::*;
pub use errors::*;
pub use event::*;
pub use platform::*;
#[cfg(test)]
pub use unit::*;
pub use utils::*;
pub use validator::*;
pub use validator_pool::*;
pub use view::*;

pub type ShareBalance = u128;

#[near(serializers = [json])]
pub struct StorageUsageInfo {
    pub per_account_storage_usage: U64,
    pub per_account_storage_cost: U128,
    pub per_conversion_storage_usage: U64,
    pub per_conversion_storage_cost: U128,
}

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    FungibleToken,
    Metadata,
    Accounts,
    Beneficiaries,
    Validators,
    PlatformUsers,
    WithdrawalRequests,
}

#[near(serializers = [borsh])]
pub struct ContractDataV1 {
    pub token: FungibleToken,
    pub metadata: LazyOption<FungibleTokenMetadata>,
    owner_id: AccountId,
    total_staked_near_amount: u128,
    accounts: IterableMap<AccountId, Account>,
    account_storage_usage: StorageUsage,
    beneficiaries: IterableMap<AccountId, u32>,
    validator_pool: ValidatorPool,
    whitelist_account_id: Option<AccountId>,
    epoch_requested_stake_amount: u128,
    epoch_requested_unstake_amount: u128,
    stake_amount_to_settle: u128,
    unstake_amount_to_settle: u128,
    last_settlement_epoch: EpochHeight,
}

#[near(serializers = [borsh])]
pub struct ContractData {
    pub token: FungibleToken,
    pub metadata: LazyOption<FungibleTokenMetadata>,
    owner_id: AccountId,
    total_staked_near_amount: u128,
    accounts: IterableMap<AccountId, Account>,
    account_storage_usage: StorageUsage,
    beneficiaries: IterableMap<AccountId, u32>,
    validator_pool: ValidatorPool,
    whitelist_account_id: Option<AccountId>,
    epoch_requested_stake_amount: u128,
    epoch_requested_unstake_amount: u128,
    stake_amount_to_settle: u128,
    unstake_amount_to_settle: u128,
    last_settlement_epoch: EpochHeight,
    wnear_id: Option<AccountId>,
    platform_users: IterableMap<AccountId, PlatformUser>,
    withdrawal_requests: IterableMap<PlatformRequestKey, WithdrawalRequest>,
    total_pending_withdrawal_amount: u128,
    refund_near_reserved: u128,
    total_platform_user_storage_deposits: u128,
    emergency_storage_reserve: u128,
    total_active_request_keys: u32,
    total_inflight_storage_operations: u32,
}

#[near(serializers = [borsh])]
pub enum VersionedContractData {
    // The original Current variant occupied index 0; keep that index for V1.
    V1(ContractDataV1),
    Current(ContractData),
}

#[derive(AccessControlRole, Deserialize, Serialize, Copy, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Role {
    DAO,
    PauseManager,
    UnpauseManager,
    UpgradableCodeStager,
    UpgradableCodeDeployer,
    OpManager,
}

#[near(contract_state)]
#[derive(Pausable, Upgradable, PanicOnDefault)]
#[access_control(role_type(Role))]
#[pausable(pause_roles(Role::PauseManager), unpause_roles(Role::UnpauseManager))]
#[upgradable(access_control_roles(
    code_stagers(Role::UpgradableCodeStager, Role::DAO),
    code_deployers(Role::UpgradableCodeDeployer, Role::DAO),
    duration_initializers(Role::DAO),
    duration_update_stagers(Role::DAO),
    duration_update_appliers(Role::DAO),
))]
pub struct Contract {
    data: VersionedContractData,
}

#[near]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId, metadata: Option<FungibleTokenMetadata>, wnear_id: AccountId) -> Self {
        let mut contract = Self {
            data: VersionedContractData::Current(ContractData {
                token: FungibleToken::new(StorageKey::FungibleToken),
                metadata: LazyOption::new(
                    StorageKey::Metadata,
                    metadata.or(Some(FungibleTokenMetadata {
                        spec: FT_METADATA_SPEC.to_string(),
                        name: "Rhea Liquid Near Staking Token".to_string(),
                        symbol: "rNEAR".to_string(),
                        icon: None,
                        reference: None,
                        reference_hash: None,
                        decimals: 24,
                    })),
                ),
                owner_id: owner_id.clone(),
                total_staked_near_amount: 0,
                accounts: IterableMap::new(StorageKey::Accounts),
                account_storage_usage: 0,
                beneficiaries: IterableMap::new(StorageKey::Beneficiaries),
                validator_pool: ValidatorPool::new(),
                whitelist_account_id: None,
                epoch_requested_stake_amount: 0,
                epoch_requested_unstake_amount: 0,
                stake_amount_to_settle: 0,
                unstake_amount_to_settle: 0,
                last_settlement_epoch: 0,
                wnear_id: Some(wnear_id),
                platform_users: IterableMap::new(StorageKey::PlatformUsers),
                withdrawal_requests: IterableMap::new(StorageKey::WithdrawalRequests),
                total_pending_withdrawal_amount: 0,
                refund_near_reserved: 0,
                total_platform_user_storage_deposits: 0,
                emergency_storage_reserve: 0,
                total_active_request_keys: 0,
                total_inflight_storage_operations: 0,
            }),
        };

        contract.measure_storage_usage();
        let contract_id = env::current_account_id();
        contract.data_mut().token.internal_register_account(&contract_id);
        contract.init_staking();

        contract.acl_init_super_admin(env::predecessor_account_id());
        contract.acl_add_super_admin(owner_id.clone());

        contract.acl_grant_role(Role::DAO.into(), owner_id.clone());
        contract.acl_grant_role(Role::PauseManager.into(), owner_id.clone());
        contract.acl_grant_role(Role::UnpauseManager.into(), owner_id.clone());
        contract.acl_grant_role(Role::OpManager.into(), owner_id.clone());

        contract
    }

    #[payable]
    pub fn donate(&mut self) {
        let amount = env::attached_deposit().as_yoctonear();
        self.data_mut().total_staked_near_amount += amount;
        // Increase requested stake amount within the current epoch
        self.data_mut().epoch_requested_stake_amount += amount;
        Event::Donate {
            account_id: env::predecessor_account_id(),
            amount: amount.into(),
        }
        .emit();
    }
}

impl Contract {
    fn available_near(&self) -> u128 {
        let storage_stake = (env::storage_usage() as u128)
            .checked_mul(env::storage_byte_cost().as_yoctonear())
            .expect("Storage stake overflow");
        let reserved = storage_stake
            .checked_add(CONTRACT_MIN_RESERVE_BALANCE.as_yoctonear())
            .and_then(|value| value.checked_add(self.data().total_platform_user_storage_deposits))
            .and_then(|value| value.checked_add(self.data().emergency_storage_reserve))
            .and_then(|value| value.checked_add(self.data().refund_near_reserved))
            .expect("NEAR reserve overflow");
        env::account_balance()
            .as_yoctonear()
            .checked_sub(reserved)
            .expect("NEAR balance is below reserved funds")
    }

    fn measure_storage_usage(&mut self) {
        let tmp_account_id = "0".repeat(64).parse().unwrap();
        let tmp_accounnt = self.internal_get_account(&tmp_account_id);
        // measure account
        {
            let initial_storage_usage = env::storage_usage();
            self.internal_save_account(&tmp_account_id, &tmp_accounnt);
            self.data_mut().accounts.flush();
            self.data_mut().account_storage_usage = self.data().token.account_storage_usage + env::storage_usage() - initial_storage_usage;
        }
        self.data_mut().accounts.remove(&tmp_account_id);
    }

    fn init_staking(&mut self) {
        require!(self.data().total_staked_near_amount == 0, ERR_ALREADY_INITIALIZED);
        let account_id = env::current_account_id();
        require!(
            self.available_near() >= INIT_STAKING_AMOUNT,
            format!("{}. required: {}", ERR_NO_ENOUGH_INIT_DEPOSIT, INIT_STAKING_AMOUNT)
        );
        self.mint_lst(&account_id, INIT_STAKING_AMOUNT, Some("init_stake"));
        self.data_mut().total_staked_near_amount += INIT_STAKING_AMOUNT;
        self.data_mut().epoch_requested_stake_amount += INIT_STAKING_AMOUNT;
    }

    #[allow(unreachable_patterns)]
    fn data(&self) -> &ContractData {
        match &self.data {
            VersionedContractData::Current(data) => data,
            _ => unimplemented!(),
        }
    }

    #[allow(unreachable_patterns)]
    fn data_mut(&mut self) -> &mut ContractData {
        match &mut self.data {
            VersionedContractData::Current(data) => data,
            _ => unimplemented!(),
        }
    }

    /// Returns the number of "stake" shares rounded down corresponding to the given staked balance
    /// amount.
    ///
    /// price = total_staked / total_shares
    /// Price is fixed
    /// (total_staked + amount) / (total_shares + num_shares) = total_staked / total_shares
    /// (total_staked + amount) * total_shares = total_staked * (total_shares + num_shares)
    /// amount * total_shares = total_staked * num_shares
    /// num_shares = amount * total_shares / total_staked
    pub(crate) fn num_shares_from_staked_amount_rounded_down(&self, amount: u128) -> ShareBalance {
        require!(self.data().total_staked_near_amount > 0, ERR_NON_POSITIVE_TOTAL_STAKED_BALANCE);
        (U256::from(self.data().token.total_supply) * U256::from(amount) / U256::from(self.data().total_staked_near_amount)).as_u128()
    }

    /// Returns the number of "stake" shares rounded up corresponding to the given staked balance
    /// amount.
    ///
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    pub(crate) fn num_shares_from_staked_amount_rounded_up(&self, amount: u128) -> ShareBalance {
        require!(self.data().total_staked_near_amount > 0, ERR_NON_POSITIVE_TOTAL_STAKED_BALANCE);
        ((U256::from(self.data().token.total_supply) * U256::from(amount) + U256::from(self.data().total_staked_near_amount - 1))
            / U256::from(self.data().total_staked_near_amount))
        .as_u128()
    }

    /// Returns the staked amount rounded down corresponding to the given number of "stake" shares.
    pub(crate) fn staked_amount_from_num_shares_rounded_down(&self, num_shares: ShareBalance) -> u128 {
        require!(self.data().token.total_supply > 0, ERR_NON_POSITIVE_TOTAL_STAKE_SHARES);
        (U256::from(self.data().total_staked_near_amount) * U256::from(num_shares) / U256::from(self.data().token.total_supply)).as_u128()
    }
}

impl Contract {
    pub fn mint_lst(&mut self, account_id: &AccountId, shares: u128, memo: Option<&str>) {
        require!(shares > 0, ERR_NON_POSITIVE_SHARES);
        // mint to account
        if self.data().token.accounts.get(account_id).is_none() {
            self.data_mut().token.internal_register_account(account_id);
        }
        self.data_mut().token.internal_deposit(account_id, shares);
        FtMint {
            owner_id: account_id,
            amount: U128(shares),
            memo,
        }
        .emit();
    }

    pub fn burn_lst(&mut self, account_id: &AccountId, shares: u128, memo: Option<&str>) {
        require!(shares > 0, ERR_NON_POSITIVE_SHARES);
        // burn from account
        self.data_mut().token.internal_withdraw(account_id, shares);
        FtBurn {
            owner_id: account_id,
            amount: U128(shares),
            memo,
        }
        .emit();
    }

    pub fn per_account_storage_cost(&self) -> u128 {
        env::storage_byte_cost()
            .checked_mul(self.data().account_storage_usage as u128)
            .unwrap()
            .as_yoctonear()
    }
}

use crate::*;

#[near]
impl Contract {
    /// A method to migrate a state during the contract upgrade.
    /// Can only be called after upgrade method.
    #[private]
    #[init(ignore_state)]
    pub fn migrate_state() -> Self {
        let mut contract: Contract = env::state_read().expect("ContractStateIsMissing");
        contract.data = match contract.data {
            VersionedContractData::V1(old) => VersionedContractData::Current(ContractData {
                token: old.token,
                metadata: old.metadata,
                owner_id: old.owner_id,
                total_staked_near_amount: old.total_staked_near_amount,
                accounts: old.accounts,
                account_storage_usage: old.account_storage_usage,
                beneficiaries: old.beneficiaries,
                validator_pool: old.validator_pool,
                whitelist_account_id: old.whitelist_account_id,
                epoch_requested_stake_amount: old.epoch_requested_stake_amount,
                epoch_requested_unstake_amount: old.epoch_requested_unstake_amount,
                stake_amount_to_settle: old.stake_amount_to_settle,
                unstake_amount_to_settle: old.unstake_amount_to_settle,
                last_settlement_epoch: old.last_settlement_epoch,
                wnear_id: None,
                platform_users: IterableMap::new(StorageKey::PlatformUsers),
                withdrawal_requests: IterableMap::new(StorageKey::WithdrawalRequests),
                total_pending_withdrawal_amount: 0,
                total_platform_user_storage_deposits: 0,
                emergency_storage_reserve: 0,
                total_active_request_keys: 0,
                total_inflight_storage_operations: 0,
                stake_lostfound: IterableMap::new(StorageKey::StakeLostFound),
                next_stake_attempt_id: 0,
                stake_lostfound_near_reserved: 0,
            }),
            VersionedContractData::Current(data) => VersionedContractData::Current(data),
        };
        let contract_id = env::current_account_id();
        if contract.data().token.accounts.get(&contract_id).is_none() {
            contract.data_mut().token.internal_register_account(&contract_id);
        }
        contract
    }

    /// Returns semver of this contract.
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

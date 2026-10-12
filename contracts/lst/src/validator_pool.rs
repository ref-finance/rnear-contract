use crate::*;
use std::cmp::{max, min, Ordering};

const STAKE_SMALL_CHANGE_AMOUNT: u128 = ONE_NEAR;
const MAX_UPDATE_WEIGHTS_COUNT: usize = 300;

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128;

    fn get_account_total_balance(&self, account_id: AccountId) -> U128;

    fn get_account(&self, account_id: AccountId) -> HumanReadableAccount;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn withdraw(&mut self, amount: U128);

    fn withdraw_all(&mut self);

    fn stake(&mut self, amount: U128);

    fn unstake(&mut self, amount: U128);

    fn unstake_all(&mut self);
}

#[ext_contract(ext_whitelist)]
pub trait ExtWhitelist {
    fn is_whitelisted(&self, staking_pool_account_id: AccountId) -> bool;
}

#[near(serializers = [borsh])]
pub struct ValidatorPool {
    pub validators: IterableMap<AccountId, VersionedValidator>,
    pub total_weight: u16,
    pub total_base_stake_amount: u128,
}

impl Default for ValidatorPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidatorPool {
    pub fn new() -> Self {
        Self {
            validators: IterableMap::new(StorageKey::Validators),
            total_weight: 0,
            total_base_stake_amount: 0,
        }
    }

    pub fn count(&self) -> u32 {
        self.validators.len()
    }

    pub fn get_validator(&self, validator_id: &AccountId) -> Option<Validator> {
        self.validators.get(validator_id).map(|v| v.into())
    }

    pub fn get_validators(
        &self,
        from_index: Option<usize>,
        limit: Option<usize>,
    ) -> Vec<Validator> {
        let len = self.validators.len() as usize;
        let skip_n = from_index.unwrap_or(0);
        let take_n = limit.unwrap_or(len - skip_n);
        self.validators
            .iter()
            .skip(skip_n)
            .take(take_n)
            .map(|(_, v)| v.into())
            .collect()
    }

    pub fn validator_target_stake_amount(
        &self,
        total_staked_near_amount: u128,
        validator: &Validator,
    ) -> u128 {
        let base_stake_amount = if total_staked_near_amount >= self.total_base_stake_amount {
            validator.base_stake_amount
        } else {
            (U256::from(validator.base_stake_amount) * U256::from(total_staked_near_amount)
                / U256::from(self.total_base_stake_amount))
            .as_u128()
        };
        // If not enough staked NEAR, satisfy the base stake amount first (set dynamic stake amount to 0)
        let dynamic_stake_amount =
            if validator.weight == 0 || total_staked_near_amount <= self.total_base_stake_amount {
                0
            } else {
                (U256::from(total_staked_near_amount - self.total_base_stake_amount)
                    * U256::from(validator.weight)
                    / U256::from(self.total_weight))
                .as_u128()
            };
        base_stake_amount + dynamic_stake_amount
    }

    pub fn get_num_epoch_to_unstake(&self, amount: u128) -> EpochHeight {
        let mut available_amount: u128 = 0;
        let mut total_staked_amount: u128 = 0;
        for validator in self.validators.values() {
            let validator: Validator = validator.into();
            total_staked_amount += validator.staked_amount;

            if !validator.pending_release()
                && !validator.executing
                && validator.weight > 0
                && validator.staked_amount > 0
            {
                available_amount += validator.staked_amount;
            }

            // found enough balance to unstake from available validators
            if available_amount >= amount {
                return NUM_EPOCHS_TO_UNLOCK;
            }
        }

        // nothing is actually staked, all balance should be available now
        // still leave a buffer for the user
        if total_staked_amount == 0 {
            return NUM_EPOCHS_TO_UNLOCK;
        }

        // no enough available validators to unstake
        // double the unstake waiting time
        2 * NUM_EPOCHS_TO_UNLOCK
    }
}

impl ValidatorPool {
    fn allocatable_staked_amount(&self, total_staked_near_amount: u128) -> u128 {
        let isolated_balance = self
            .validators
            .values()
            .map(Validator::from)
            .filter(|validator| validator.weight == 0)
            .fold(0_u128, |total, validator| {
                total.saturating_add(
                    validator
                        .staked_amount
                        .saturating_add(validator.unstaked_amount),
                )
            });
        total_staked_near_amount.saturating_sub(isolated_balance)
    }

    pub fn get_candidate_to_stake(
        &self,
        amount: u128,
        total_staked_near_amount: u128,
    ) -> Option<CandidateValidator> {
        let mut candidate = None;
        let mut max_delta: u128 = 0;

        for (_, validator) in self.validators.iter() {
            let validator = validator.into();
            let target_amount =
                self.validator_target_stake_amount(total_staked_near_amount, &validator);
            if validator.staked_amount < target_amount {
                let delta = target_amount - validator.staked_amount;
                if delta > max_delta {
                    max_delta = delta;
                    candidate = Some(validator);
                }
            }
        }

        let mut amount_to_stake: u128 = min(amount, max_delta);

        if amount_to_stake > 0 && amount - amount_to_stake < STAKE_SMALL_CHANGE_AMOUNT {
            amount_to_stake = amount;
        }

        candidate.map(|candidate| CandidateValidator {
            validator: candidate,
            amount: amount_to_stake,
        })
    }

    fn filter_candidate_validators(
        &self,
        allocatable_amount: u128,
    ) -> Vec<(Validator, u128, u128)> {
        self.validators
            .values()
            .map(|versioned_validator| {
                let validator = Validator::from(versioned_validator);
                let target_amount =
                    self.validator_target_stake_amount(allocatable_amount, &validator);
                (validator, target_amount)
            })
            .filter(|(validator, target_amount)| {
                // validator is not in pending release
                !validator.pending_release()
                    && !validator.executing
                    && validator.weight > 0
                    // delta must > 0
                    && validator.staked_amount > *target_amount
            })
            .map(|(validator, target_amount)| {
                let delta = validator.staked_amount - target_amount; // safe sub
                (validator, target_amount, delta)
            })
            .collect()
    }

    // Sort candidate validators by delta in ascending order
    fn sort_candidate_validators_by_delta_asc(
        candidate_validators: &mut [(Validator, u128, u128)],
    ) {
        candidate_validators.sort_by(
            |(_validator_1, _target_amount_1, delta_1),
             (_validator_2, _target_amount_2, delta_2)| { delta_1.cmp(delta_2) },
        );
    }

    // Sort candidate validators by (delta / target) in descending order
    fn sort_candidate_validators_by_ratio_of_delta_to_target_desc(
        candidate_validators: &mut [(Validator, u128, u128)],
    ) {
        candidate_validators.sort_by(
            |(_validator_1, target_amount_1, delta_1), (_validator_2, target_amount_2, delta_2)| {
                let target_amount_1 = *target_amount_1;
                let target_amount_2 = *target_amount_2;

                if target_amount_1 == 0 && target_amount_2 == 0 {
                    delta_2.cmp(delta_1)
                } else if target_amount_1 != 0 && target_amount_2 == 0 {
                    Ordering::Greater
                } else if target_amount_1 == 0 && target_amount_2 != 0 {
                    Ordering::Less
                } else {
                    // We can simplify `(delta_2 / target_amount_2) cmp (delta_1 / target_amount_1)`
                    // to `(delta_2 * target_amount_1) cmp (delta_1 * target_amount_2)`
                    let mul_1 = U256::from(*delta_1) * U256::from(target_amount_2);
                    let mul_2 = U256::from(*delta_2) * U256::from(target_amount_1);
                    match mul_2.cmp(&mul_1) {
                        Ordering::Equal => delta_2.cmp(delta_1),
                        Ordering::Less => Ordering::Less,
                        Ordering::Greater => Ordering::Greater,
                    }
                }
            },
        );
    }

    pub fn get_candidate_to_unstake_v2(
        &self,
        total_amount_to_unstake: u128,
        total_staked_near_amount: u128,
    ) -> Option<CandidateValidator> {
        let allocatable_amount = self.allocatable_staked_amount(total_staked_near_amount);
        let mut candidate_validators = self.filter_candidate_validators(allocatable_amount);
        if candidate_validators.is_empty() {
            return None;
        }

        Self::sort_candidate_validators_by_delta_asc(&mut candidate_validators);

        let candidate = candidate_validators
            .iter()
            .find(|(_validator, _target_amount, delta)| *delta >= total_amount_to_unstake);

        if let Some((validator, _target_amount, _delta)) = candidate {
            return Some(CandidateValidator {
                validator: validator.clone(),
                amount: total_amount_to_unstake,
            });
        };

        Self::sort_candidate_validators_by_ratio_of_delta_to_target_desc(&mut candidate_validators);

        candidate_validators
            .first()
            .map(|(validator, target_amount, delta)| {
                let amount_to_unstake = min3(
                    // unstake no more than total requirement
                    total_amount_to_unstake,
                    max(target_amount / 2, *delta),
                    // guaranteed minimum staked amount even if `total_staked_near_amount` is less than `total_base_stake_amount`
                    validator.staked_amount.saturating_sub(min(
                        (U256::from(validator.base_stake_amount) * U256::from(allocatable_amount))
                            .checked_div(U256::from(self.total_base_stake_amount))
                            .unwrap_or_default()
                            .as_u128(),
                        validator.base_stake_amount,
                    )),
                );
                CandidateValidator {
                    validator: validator.clone(),
                    amount: amount_to_unstake,
                }
            })
    }
}

impl ValidatorPool {
    pub fn save_validator(&mut self, validator: &Validator) {
        self.validators
            .insert(validator.account_id.clone(), validator.clone().into());
    }

    pub fn add_validator(&mut self, validator_id: &AccountId, weight: u16) -> Validator {
        require!(
            self.get_validator(validator_id).is_none(),
            ERR_VALIDATOR_ALREADY_EXIST
        );

        let validator = Validator::new(validator_id.clone(), weight);

        self.validators
            .insert(validator_id.clone(), validator.clone().into());

        self.total_weight += weight;

        Event::ValidatorAdded {
            account_id: validator_id,
            weight,
        }
        .emit();

        validator
    }

    pub fn remove_validator(&mut self, validator_id: &AccountId) -> Validator {
        let validator: Validator = self
            .validators
            .remove(validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST)
            .into();

        // make sure this validator is not used at all
        require!(
            validator.staked_amount == 0 && validator.unstaked_amount == 0 && !validator.executing,
            ERR_VALIDATOR_IN_USE
        );

        self.total_weight -= validator.weight;
        self.total_base_stake_amount -= validator.base_stake_amount;

        Event::ValidatorRemoved {
            account_id: validator_id,
        }
        .emit();

        validator
    }

    pub fn update_weight(&mut self, validator_id: &AccountId, weight: u16) -> u16 {
        let mut validator: Validator = self
            .validators
            .get(validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST)
            .into();

        let old_weight = validator.weight;
        // update total weight
        self.total_weight = self.total_weight + weight - old_weight;

        validator.weight = weight;
        self.validators
            .insert(validator_id.clone(), validator.into());

        old_weight
    }

    pub fn update_base_stake_amount(&mut self, validator_id: &AccountId, amount: u128) {
        let mut validator: Validator = self
            .validators
            .get(validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST)
            .into();

        let old_base_stake_amount = validator.base_stake_amount;
        // update total base stake amount
        self.total_base_stake_amount =
            self.total_base_stake_amount + amount - old_base_stake_amount;

        validator.base_stake_amount = amount;
        self.validators
            .insert(validator_id.clone(), validator.into());

        Event::ValidatorUpdatedBaseStakeAmount {
            account_id: validator_id,
            old_base_stake_amount: &old_base_stake_amount.into(),
            new_base_stake_amount: &amount.into(),
        }
        .emit();
    }
}

#[near]
impl Contract {
    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn add_validator(&mut self, validator_id: AccountId, weight: u16) {
        assert_one_yocto();
        self.add_whitelisted_validator(&validator_id, weight);
    }

    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn add_validators(&mut self, validator_ids: Vec<AccountId>, weights: Vec<u16>) {
        assert_one_yocto();
        require!(validator_ids.len() == weights.len(), ERR_BAD_VALIDATOR_LIST);
        for i in 0..validator_ids.len() {
            self.add_whitelisted_validator(&validator_ids[i], weights[i]);
        }
    }

    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn remove_validator(&mut self, validator_id: AccountId) -> Validator {
        assert_one_yocto();
        self.data_mut()
            .validator_pool
            .remove_validator(&validator_id)
    }

    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn update_weight(&mut self, validator_id: AccountId, weight: u16) {
        assert_one_yocto();
        let old_weight = self
            .data_mut()
            .validator_pool
            .update_weight(&validator_id, weight);
        Event::ValidatorsUpdatedWeights {
            account_ids: vec![&validator_id],
            old_weights: vec![old_weight],
            new_weights: vec![weight],
        }
        .emit();
    }

    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn update_weights(&mut self, validator_ids: Vec<AccountId>, weights: Vec<u16>) {
        assert_one_yocto();
        require!(validator_ids.len() == weights.len(), ERR_BAD_VALIDATOR_LIST);

        require!(
            validator_ids.len() <= MAX_UPDATE_WEIGHTS_COUNT,
            format!(
                "The number of validators to be updated at a time cannot exceed {}",
                MAX_UPDATE_WEIGHTS_COUNT
            )
        );

        let mut account_ids = Vec::new();
        let mut old_weights = Vec::new();
        let mut new_weights = Vec::new();

        for i in 0..validator_ids.len() {
            let old_weight = self
                .data_mut()
                .validator_pool
                .update_weight(&validator_ids[i], weights[i]);
            account_ids.push(&validator_ids[i]);
            old_weights.push(old_weight);
            new_weights.push(weights[i]);
        }

        Event::ValidatorsUpdatedWeights {
            account_ids,
            old_weights,
            new_weights,
        }
        .emit();
    }

    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn update_base_stake_amounts(&mut self, validator_ids: Vec<AccountId>, amounts: Vec<U128>) {
        assert_one_yocto();
        require!(validator_ids.len() == amounts.len(), ERR_BAD_VALIDATOR_LIST);
        for i in 0..validator_ids.len() {
            self.data_mut()
                .validator_pool
                .update_base_stake_amount(&validator_ids[i], amounts[i].into());
        }
    }

    #[pause]
    pub fn sync_balance_from_validator(&mut self, validator_id: AccountId) {
        let min_gas = GAS_SYNC_BALANCE.as_gas()
            + GAS_EXT_GET_ACCOUNT.as_gas()
            + GAS_CB_VALIDATOR_SYNC_BALANCE.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST);

        validator
            .sync_account_balance(&mut self.data_mut().validator_pool, false)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CB_VALIDATOR_SYNC_BALANCE)
                    .with_unused_gas_weight(0)
                    .validator_get_account_callback(validator.account_id),
            );
    }

    /// This method is designed to drain a validator.
    /// The weight of target validator should be set to 0 before calling this.
    /// And a following call to drain_withdraw MUST be made after 4 epochs.
    #[payable]
    #[access_control_any(roles(Role::OpManager, Role::DAO))]
    #[pause]
    pub fn drain_unstake(&mut self, validator_id: AccountId) -> Promise {
        assert_one_yocto();
        // make sure enough gas was given
        let min_gas = GAS_DRAIN_UNSTAKE.as_gas()
            + GAS_EXT_UNSTAKE.as_gas()
            + GAS_CB_VALIDATOR_UNSTAKED.as_gas()
            + GAS_SYNC_BALANCE.as_gas()
            + GAS_CB_VALIDATOR_SYNC_BALANCE.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST);

        // make sure the validator:
        // 1. has weight set to 0
        // 2. has base stake amount set to 0
        // 3. not in pending release
        // 4. has not unstaked balance (because this part is from user's unstake request)
        // 5. not in draining process
        require!(validator.weight == 0, ERR_NON_ZERO_WEIGHT);
        require!(
            validator.base_stake_amount == 0,
            ERR_NON_ZERO_BASE_STAKE_AMOUNT
        );
        require!(
            !validator.pending_release(),
            ERR_VALIDATOR_UNSTAKE_WHEN_LOCKED
        );
        // in practice we allow 1 NEAR due to the precision of stake operation
        require!(
            validator.unstaked_amount < ONE_NEAR,
            ERR_BAD_UNSTAKED_AMOUNT
        );
        require!(!validator.draining, ERR_DRAINING);

        let unstake_amount = validator.staked_amount;

        Event::DrainUnstakeAttempt {
            validator_id: &validator_id,
            amount: &U128(unstake_amount),
        }
        .emit();

        // perform actual unstake
        validator
            .unstake(&mut self.data_mut().validator_pool, unstake_amount)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(
                        GAS_CB_VALIDATOR_UNSTAKED
                            .checked_add(GAS_SYNC_BALANCE)
                            .unwrap()
                            .checked_add(GAS_CB_VALIDATOR_SYNC_BALANCE)
                            .unwrap(),
                    )
                    .with_unused_gas_weight(0)
                    .validator_drain_unstaked_callback(validator.account_id, unstake_amount.into()),
            )
    }

    /// Withdraw from a drained validator
    #[pause]
    pub fn drain_withdraw(&mut self, validator_id: AccountId) {
        // make sure enough gas was given
        let min_gas = GAS_DRAIN_WITHDRAW.as_gas()
            + GAS_EXT_WITHDRAW.as_gas()
            + GAS_CB_VALIDATOR_WITHDRAW.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .expect(ERR_VALIDATOR_NOT_EXIST);

        // make sure the validator:
        // 1. has weight set to 0
        // 2. has base stake amount set to 0
        // 3. has no staked balance
        // 4. not pending release
        // 5. in draining process
        require!(validator.weight == 0, ERR_NON_ZERO_WEIGHT);
        require!(
            validator.base_stake_amount == 0,
            ERR_NON_ZERO_BASE_STAKE_AMOUNT
        );
        require!(validator.staked_amount == 0, ERR_NON_ZERO_STAKED_AMOUNT);
        require!(
            !validator.pending_release(),
            ERR_VALIDATOR_WITHDRAW_WHEN_LOCKED
        );
        require!(validator.draining, ERR_NOT_IN_DRAINING);

        let amount = validator.unstaked_amount;

        Event::DrainWithdrawAttempt {
            validator_id: &validator_id,
            amount: &U128(amount),
        }
        .emit();

        validator
            .withdraw(&mut self.data_mut().validator_pool, amount)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CB_VALIDATOR_WITHDRAW)
                    .with_unused_gas_weight(0)
                    .validator_drain_withdraw_callback(validator.account_id.clone(), amount.into()),
            );
    }
}

#[near]
impl Contract {
    #[private]
    pub fn is_whitelisted_callback(
        &mut self,
        validator_id: AccountId,
        weight: u16,
        #[callback] whitelisted: bool,
    ) {
        require!(
            whitelisted,
            format!(
                "{}. {}",
                ERR_VALIDATOR_NOT_WHITELISTED,
                validator_id.clone()
            )
        );

        self.data_mut()
            .validator_pool
            .add_validator(&validator_id, weight);
    }
}

#[near]
impl Contract {
    #[private]
    pub fn validator_drain_unstaked_callback(
        &mut self,
        validator_id: AccountId,
        amount: U128,
    ) -> PromiseOrValue<()> {
        let amount = amount.into();
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        if is_promise_success() {
            validator.on_unstake_success(&mut self.data_mut().validator_pool, amount);
            validator.set_draining(&mut self.data_mut().validator_pool, true);

            Event::DrainUnstakeSuccess {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            validator
                .sync_account_balance(&mut self.data_mut().validator_pool, true)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CB_VALIDATOR_SYNC_BALANCE)
                        .with_unused_gas_weight(0)
                        .validator_get_account_callback(validator_id),
                )
                .into()
        } else {
            // unstake failed, revert
            validator.on_unstake_failed(&mut self.data_mut().validator_pool);

            Event::DrainUnstakeFailed {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            PromiseOrValue::Value(())
        }
    }

    #[private]
    pub fn validator_drain_withdraw_callback(&mut self, validator_id: AccountId, amount: U128) {
        let amount = amount.into();
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        if is_promise_success() {
            validator.on_withdraw_success(&mut self.data_mut().validator_pool);
            validator.set_draining(&mut self.data_mut().validator_pool, false);

            Event::DrainWithdrawSuccess {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            // those funds need to be restaked, so we add them back to epoch request
            self.data_mut().epoch_requested_stake_amount += amount;
        } else {
            // withdraw failed, revert
            validator.on_withdraw_failed(&mut self.data_mut().validator_pool, amount);

            Event::DrainWithdrawFailed {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();
        }
    }
}

impl Contract {
    fn add_whitelisted_validator(&mut self, validator_id: &AccountId, weight: u16) {
        let whitelist_id = self
            .data()
            .whitelist_account_id
            .as_ref()
            .expect(ERR_VALIDATOR_WHITELIST_NOT_SET);

        ext_whitelist::ext(whitelist_id.clone())
            .with_static_gas(GAS_EXT_WHITELIST)
            .with_unused_gas_weight(0)
            .is_whitelisted(validator_id.clone())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CB_WHITELIST)
                    .with_unused_gas_weight(0)
                    .is_whitelisted_callback(validator_id.clone(), weight),
            );
    }
}

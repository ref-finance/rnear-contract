use crate::*;

const MIN_AMOUNT_TO_PERFORM_STAKE: u128 = NearToken::from_near(1).as_yoctonear();
const MAX_SYNC_BALANCE_DIFF: u128 = 100;

impl Contract {
    /// Cleaning up stake requirements and unstake requirements,
    /// since some stake requirements could be eliminated if
    /// there are more unstake requirements, and vice versa.
    fn epoch_cleanup(&mut self) {
        if self.data().last_settlement_epoch == get_epoch_height() {
            return;
        }
        self.data_mut().last_settlement_epoch = get_epoch_height();

        // here we use += because cleanup amount might not be 0
        self.data_mut().stake_amount_to_settle += self.data().epoch_requested_stake_amount;
        self.data_mut().unstake_amount_to_settle += self.data().epoch_requested_unstake_amount;
        self.data_mut().epoch_requested_stake_amount = 0;
        self.data_mut().epoch_requested_unstake_amount = 0;

        if self.data().stake_amount_to_settle > self.data().unstake_amount_to_settle {
            self.data_mut().stake_amount_to_settle -= self.data().unstake_amount_to_settle;
            self.data_mut().unstake_amount_to_settle = 0;
        } else {
            self.data_mut().unstake_amount_to_settle -= self.data().stake_amount_to_settle;
            self.data_mut().stake_amount_to_settle = 0;
        }

        Event::EpochCleanup {
            stake_amount_to_settle: &U128(self.data().stake_amount_to_settle),
            unstake_amount_to_settle: &U128(self.data().unstake_amount_to_settle),
        }
        .emit();
    }
}

#[near]
impl Contract {
    /// Stake $NEAR to one of the validators.
    ///
    /// Select a candidate validator and stake part of or all of the to-settle
    /// stake amounts to this validator. This function is expected to be called
    /// in each epoch.
    ///
    /// # Return
    /// * `true` - a candidate validator is selected and successfully staked to.
    ///            There might be more stake amounts to settle so this function
    ///            should be called again.
    /// * `false` - There is no need to call this function again in this epoch.
    #[pause]
    pub fn epoch_stake(&mut self) -> PromiseOrValue<bool> {
        // make sure enough gas was given
        let min_gas = GAS_EPOCH_STAKE.as_gas()
            + GAS_EXT_DEPOSIT_AND_STAKE.as_gas()
            + GAS_CB_VALIDATOR_STAKED.as_gas()
            + GAS_SYNC_BALANCE.as_gas()
            + GAS_CB_VALIDATOR_SYNC_BALANCE.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        self.epoch_cleanup();
        // after cleanup, there might be no need to stake
        if self.data().stake_amount_to_settle == 0 {
            log!("no need to stake, amount to settle is zero");
            return PromiseOrValue::Value(false);
        }

        let candidate = self
            .data()
            .validator_pool
            .get_candidate_to_stake(self.data().stake_amount_to_settle, self.data().total_staked_near_amount);

        if candidate.is_none() {
            log!("no candidate found to stake");
            return PromiseOrValue::Value(false);
        }

        let mut candidate = candidate.unwrap();
        let amount_to_stake = candidate.amount;

        if amount_to_stake < MIN_AMOUNT_TO_PERFORM_STAKE {
            log!("stake amount too low: {}", amount_to_stake);
            return PromiseOrValue::Value(false);
        }

        require!(self.available_near() >= amount_to_stake, ERR_MIN_RESERVE);

        // update internal state
        self.data_mut().stake_amount_to_settle -= amount_to_stake;

        Event::EpochStakeAttempt {
            validator_id: &candidate.validator.account_id,
            amount: &U128(amount_to_stake),
        }
        .emit();

        // do staking on selected validator
        candidate
            .validator
            .deposit_and_stake(&mut self.data_mut().validator_pool, amount_to_stake)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(
                        GAS_CB_VALIDATOR_STAKED
                            .checked_add(GAS_SYNC_BALANCE)
                            .unwrap()
                            .checked_add(GAS_CB_VALIDATOR_SYNC_BALANCE)
                            .unwrap(),
                    )
                    .validator_staked_callback(candidate.validator.account_id.clone(), amount_to_stake.into()),
            )
            .into()
    }

    /// Unstake $NEAR from one of the validators.
    ///
    /// Select a candidate validator and unstake part of or all of the to-settle
    /// unstake amounts from this validator. This function is expected to be called
    /// in each epoch.
    ///
    /// # Return
    /// * `true` - a candidate validator is selected and successfully unstaked from.
    ///            There might be more unstake amounts to settle so this function
    ///            should be called again.
    /// * `false` - There is no need to call this function again in this epoch.
    #[pause]
    pub fn epoch_unstake(&mut self) -> PromiseOrValue<bool> {
        // make sure enough gas was given
        let min_gas = GAS_EPOCH_UNSTAKE.as_gas()
            + GAS_EXT_UNSTAKE.as_gas()
            + GAS_CB_VALIDATOR_UNSTAKED.as_gas()
            + GAS_SYNC_BALANCE.as_gas()
            + GAS_CB_VALIDATOR_SYNC_BALANCE.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        self.epoch_cleanup();
        // after cleanup, there might be no need to unstake
        if self.data().unstake_amount_to_settle == 0 {
            log!("no need to unstake, amount to settle is zero");
            return PromiseOrValue::Value(false);
        }

        let candidate = self
            .data()
            .validator_pool
            .get_candidate_to_unstake_v2(self.data().unstake_amount_to_settle, self.data().total_staked_near_amount);
        if candidate.is_none() {
            log!("no candidate found to unstake");
            return PromiseOrValue::Value(false);
        }
        let mut candidate = candidate.unwrap();
        let amount_to_unstake = candidate.amount;

        // Since it's reasonable to unstake any amount of NEAR from a validator, as low as 1 yocto NEAR,
        // when its target stake amount is 0, here we don't enforce the minimun unstake amount requirement.

        // update internal state
        self.data_mut().unstake_amount_to_settle -= amount_to_unstake;

        Event::EpochUnstakeAttempt {
            validator_id: &candidate.validator.account_id,
            amount: &U128(amount_to_unstake),
        }
        .emit();

        // do unstaking on selected validator
        candidate
            .validator
            .unstake(&mut self.data_mut().validator_pool, amount_to_unstake)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(
                        GAS_CB_VALIDATOR_UNSTAKED
                            .checked_add(GAS_SYNC_BALANCE)
                            .unwrap()
                            .checked_add(GAS_CB_VALIDATOR_SYNC_BALANCE)
                            .unwrap(),
                    )
                    .validator_unstaked_callback(candidate.validator.account_id, amount_to_unstake.into()),
            )
            .into()
    }

    #[pause]
    pub fn epoch_update_rewards(&mut self, validator_id: AccountId) {
        let min_gas = GAS_EPOCH_UPDATE_REWARDS.as_gas() + GAS_EXT_GET_BALANCE.as_gas() + GAS_CB_VALIDATOR_GET_BALANCE.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        let mut validator = self.data_mut().validator_pool.get_validator(&validator_id).expect(ERR_VALIDATOR_NOT_EXIST);

        validator.refresh_total_balance(&mut self.data_mut().validator_pool).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CB_VALIDATOR_GET_BALANCE)
                .validator_get_balance_callback(validator.account_id),
        );
    }

    #[pause]
    pub fn epoch_withdraw(&mut self, validator_id: AccountId) {
        // make sure enough gas was given
        let min_gas = GAS_EPOCH_WITHDRAW.as_gas() + GAS_EXT_WITHDRAW.as_gas() + GAS_CB_VALIDATOR_WITHDRAW.as_gas();
        require!(
            env::prepaid_gas().as_gas() >= min_gas,
            format!("{}. require at least {:?}", ERR_NO_ENOUGH_GAS, min_gas)
        );

        let mut validator = self.data_mut().validator_pool.get_validator(&validator_id).expect(ERR_VALIDATOR_NOT_EXIST);

        require!(!validator.draining, ERR_DRAINING);

        let amount = validator.unstaked_amount;

        Event::EpochWithdrawAttempt {
            validator_id: &validator_id,
            amount: &U128(amount),
        }
        .emit();

        validator.withdraw(&mut self.data_mut().validator_pool, amount).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CB_VALIDATOR_WITHDRAW)
                .validator_withdraw_callback(validator.account_id.clone(), amount.into()),
        );
    }
}

/// callbacks
#[near]
impl Contract {
    /// # Return
    /// * `true` - Stake and sync balance succeed
    /// * `false` - Stake fails
    #[private]
    pub fn validator_staked_callback(&mut self, validator_id: AccountId, amount: U128) -> PromiseOrValue<bool> {
        let amount = amount.into();
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        if is_promise_success() {
            validator.on_stake_success(&mut self.data_mut().validator_pool, amount);

            Event::EpochStakeSuccess {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            validator
                .sync_account_balance(&mut self.data_mut().validator_pool, true)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CB_VALIDATOR_SYNC_BALANCE)
                        .validator_get_account_callback(validator_id),
                )
                .into()
        } else {
            validator.on_stake_failed(&mut self.data_mut().validator_pool);

            // stake failed, revert
            self.data_mut().stake_amount_to_settle += amount;

            Event::EpochStakeFailed {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            PromiseOrValue::Value(false)
        }
    }

    /// # Return
    /// * `true` - Unstake and sync balance succeed
    /// * `false` - Unstake fails
    #[private]
    pub fn validator_unstaked_callback(&mut self, validator_id: AccountId, amount: U128) -> PromiseOrValue<bool> {
        let amount = amount.into();
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        if is_promise_success() {
            validator.on_unstake_success(&mut self.data_mut().validator_pool, amount);

            Event::EpochUnstakeSuccess {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            validator
                .sync_account_balance(&mut self.data_mut().validator_pool, true)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CB_VALIDATOR_SYNC_BALANCE)
                        .validator_get_account_callback(validator_id),
                )
                .into()
        } else {
            // unstake failed, revert
            // 1. revert contract states
            self.data_mut().unstake_amount_to_settle += amount;

            // 2. revert validator states
            validator.on_unstake_failed(&mut self.data_mut().validator_pool);

            Event::EpochUnstakeFailed {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();

            PromiseOrValue::Value(false)
        }
    }

    #[private]
    pub fn validator_get_balance_callback(&mut self, validator_id: AccountId, #[callback_result] call_result: Result<U128, PromiseError>) {
        let mut validator = self.data_mut().validator_pool.get_validator(&validator_id).expect(ERR_VALIDATOR_NOT_EXIST);
        match call_result {
            Ok(total_balance) => {
                let new_balance = total_balance.0;
                let rewards = new_balance - validator.total_balance();
                Event::EpochUpdateRewards {
                    validator_id: &validator_id,
                    old_balance: &U128(validator.total_balance()),
                    new_balance: &U128(new_balance),
                    rewards: &U128(rewards),
                }
                .emit();

                validator.on_new_total_balance(&mut self.data_mut().validator_pool, new_balance);

                if rewards == 0 {
                    return;
                }

                self.data_mut().total_staked_near_amount += rewards;
                self.internal_distribute_staking_rewards(rewards);
            }
            Err(_) => {
                validator.on_get_account_total_balance_failed(&mut self.data_mut().validator_pool);
            }
        }
    }

    /// Callback after get the contract account balance from the validator
    ///
    /// Params:
    /// - validator_id: the validator to sync balance
    #[private]
    pub fn validator_get_account_callback(&mut self, validator_id: AccountId, #[callback_result] result: Result<HumanReadableAccount, PromiseError>) -> bool {
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        match result {
            Ok(account) => {
                // allow at most max_sync_balance_diff diff in total balance, staked balance and unstake balance
                let new_total_balance = account.staked_balance.0 + account.unstaked_balance.0;
                if abs_diff_eq(new_total_balance, validator.total_balance(), MAX_SYNC_BALANCE_DIFF)
                    && abs_diff_eq(account.staked_balance.0, validator.staked_amount, MAX_SYNC_BALANCE_DIFF)
                    && abs_diff_eq(account.unstaked_balance.0, validator.unstaked_amount, MAX_SYNC_BALANCE_DIFF)
                {
                    Event::SyncValidatorBalanceSuccess {
                        validator_id: &validator_id,
                        old_staked_balance: &validator.staked_amount.into(),
                        old_unstaked_balance: &validator.unstaked_amount.into(),
                        old_total_balance: &validator.total_balance().into(),
                        new_staked_balance: &account.staked_balance,
                        new_unstaked_balance: &account.unstaked_balance,
                        new_total_balance: &new_total_balance.into(),
                    }
                    .emit();
                    validator.on_sync_account_balance_success(&mut self.data_mut().validator_pool, account.staked_balance.0, account.unstaked_balance.0);
                } else {
                    Event::SyncValidatorBalanceFailedLargeDiff {
                        validator_id: &validator_id,
                        old_staked_balance: &validator.staked_amount.into(),
                        old_unstaked_balance: &validator.unstaked_amount.into(),
                        old_total_balance: &validator.total_balance().into(),
                        new_staked_balance: &account.staked_balance,
                        new_unstaked_balance: &account.unstaked_balance,
                        new_total_balance: &new_total_balance.into(),
                    }
                    .emit();
                    validator.on_sync_account_balance_failed(&mut self.data_mut().validator_pool);
                }
            }
            Err(_) => {
                Event::SyncValidatorBalanceFailedCannotGetAccount {
                    validator_id: &validator_id,
                    old_staked_balance: &validator.staked_amount.into(),
                    old_unstaked_balance: &validator.unstaked_amount.into(),
                    old_total_balance: &validator.total_balance().into(),
                }
                .emit();
                validator.on_sync_account_balance_failed(&mut self.data_mut().validator_pool);
            }
        };
        true
    }

    #[private]
    pub fn validator_withdraw_callback(&mut self, validator_id: AccountId, amount: U128) {
        let amount = amount.into();
        let mut validator = self
            .data_mut()
            .validator_pool
            .get_validator(&validator_id)
            .unwrap_or_else(|| panic!("{}: {}", ERR_VALIDATOR_NOT_EXIST, &validator_id));

        if is_promise_success() {
            validator.on_withdraw_success(&mut self.data_mut().validator_pool);

            Event::EpochWithdrawSuccess {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();
        } else {
            // withdraw failed, revert
            validator.on_withdraw_failed(&mut self.data_mut().validator_pool, amount);

            Event::EpochWithdrawFailed {
                validator_id: &validator_id,
                amount: &U128(amount),
            }
            .emit();
        }
    }
}

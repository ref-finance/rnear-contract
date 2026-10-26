use crate::*;

pub struct CandidateValidator {
    pub validator: Validator,
    pub amount: u128,
}

#[near(serializers = [borsh])]
pub enum VersionedValidator {
    Current(Validator),
}

impl From<Validator> for VersionedValidator {
    fn from(v: Validator) -> Self {
        VersionedValidator::Current(v)
    }
}

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Validator {
    pub account_id: AccountId,
    pub weight: u16,

    pub staked_amount: u128,
    pub unstaked_amount: u128,

    /// The base stake amount on this validator.
    pub base_stake_amount: u128,

    /// the epoch num when latest unstake action happened on this validator
    pub unstake_fired_epoch: EpochHeight,
    /// this is to save the last value of unstake_fired_epoch,
    /// so that when unstake revert we can restore it
    pub last_unstake_fired_epoch: EpochHeight,

    /// Whether the validator is in draining process
    pub draining: bool,
    /// Whether the validator is executing actions
    pub executing: bool,
}

impl From<&VersionedValidator> for Validator {
    fn from(value: &VersionedValidator) -> Self {
        match value {
            VersionedValidator::Current(v) => v.clone(),
        }
    }
}

impl From<VersionedValidator> for Validator {
    fn from(value: VersionedValidator) -> Self {
        match value {
            VersionedValidator::Current(v) => v.clone(),
        }
    }
}

#[near(serializers = [json])]
pub struct ValidatorInfo {
    pub account_id: AccountId,
    pub weight: u16,
    pub base_stake_amount: U128,
    pub target_stake_amount: U128,
    pub staked_amount: U128,
    pub unstaked_amount: U128,
    pub pending_release: bool,
    pub draining: bool,
}

impl Validator {
    pub fn new(account_id: AccountId, weight: u16) -> Self {
        Self {
            account_id,
            weight,
            base_stake_amount: 0,
            staked_amount: 0,
            unstaked_amount: 0,
            unstake_fired_epoch: 0,
            last_unstake_fired_epoch: 0,
            draining: false,
            executing: false,
        }
    }

    pub fn get_info(&self, pool: &ValidatorPool, total_staked_near_amount: u128) -> ValidatorInfo {
        ValidatorInfo {
            account_id: self.account_id.clone(),
            weight: self.weight,
            base_stake_amount: self.base_stake_amount.into(),
            target_stake_amount: pool
                .validator_target_stake_amount(total_staked_near_amount, self)
                .into(),
            staked_amount: self.staked_amount.into(),
            unstaked_amount: self.unstaked_amount.into(),
            pending_release: self.pending_release(),
            draining: self.draining,
        }
    }

    pub fn total_balance(&self) -> u128 {
        self.staked_amount + self.unstaked_amount
    }

    /// whether the validator is in unstake releasing period.
    pub fn pending_release(&self) -> bool {
        let current_epoch = get_epoch_height();
        current_epoch >= self.unstake_fired_epoch
            && current_epoch < self.unstake_fired_epoch + NUM_EPOCHS_TO_UNLOCK
    }

    pub fn deposit_and_stake(&mut self, pool: &mut ValidatorPool, amount: u128) -> Promise {
        self.pre_execution(pool);

        ext_staking_pool::ext(self.account_id.clone())
            .with_attached_deposit(NearToken::from_yoctonear(amount))
            .with_static_gas(GAS_EXT_DEPOSIT_AND_STAKE)
            .deposit_and_stake()
    }

    pub fn on_stake_success(&mut self, pool: &mut ValidatorPool, amount: u128) {
        // Do not call post_execution() here because we need to sync account balance after stake
        self.staked_amount += amount;
        pool.save_validator(self);
    }

    pub fn on_stake_failed(&mut self, pool: &mut ValidatorPool) {
        self.post_execution(pool);
    }

    pub fn unstake(&mut self, pool: &mut ValidatorPool, amount: u128) -> Promise {
        require!(
            amount <= self.staked_amount,
            format!(
                "{}. staked: {}, requested: {}",
                ERR_VALIDATOR_UNSTAKE_AMOUNT, self.staked_amount, amount
            )
        );

        self.begin_unstake(pool);

        ext_staking_pool::ext(self.account_id.clone())
            .with_static_gas(GAS_EXT_UNSTAKE)
            .unstake(amount.into())
    }

    pub fn unstake_all(&mut self, pool: &mut ValidatorPool) -> Promise {
        require!(self.staked_amount > 0, ERR_NON_POSITIVE_UNSTAKING_AMOUNT);
        self.begin_unstake(pool);

        ext_staking_pool::ext(self.account_id.clone())
            .with_static_gas(GAS_EXT_UNSTAKE)
            .unstake_all()
    }

    fn begin_unstake(&mut self, pool: &mut ValidatorPool) {
        require!(!self.pending_release(), ERR_VALIDATOR_UNSTAKE_WHEN_LOCKED);
        self.pre_execution(pool);

        self.last_unstake_fired_epoch = self.unstake_fired_epoch;
        self.unstake_fired_epoch = get_epoch_height();

        pool.save_validator(self);
    }

    pub fn on_unstake_success(&mut self, pool: &mut ValidatorPool, amount: u128) {
        // Do not call post_execution() here because we need to sync account balance after unstake
        self.staked_amount -= amount;
        self.unstaked_amount += amount;
        pool.save_validator(self);
    }

    pub fn on_unstake_failed(&mut self, pool: &mut ValidatorPool) {
        self.post_execution(pool);

        self.unstake_fired_epoch = self.last_unstake_fired_epoch;
        pool.save_validator(self);
    }

    pub fn refresh_total_balance(&mut self, pool: &mut ValidatorPool) -> Promise {
        self.pre_execution(pool);

        ext_staking_pool::ext(self.account_id.clone())
            .with_static_gas(GAS_EXT_GET_BALANCE)
            .get_account_total_balance(env::current_account_id())
    }

    pub fn on_new_total_balance(&mut self, pool: &mut ValidatorPool, new_total_balance: u128) {
        self.post_execution(pool);

        // sync base stake amount
        self.sync_base_stake_amount(pool, new_total_balance);
        // update staked amount
        self.staked_amount = new_total_balance - self.unstaked_amount;
        pool.save_validator(self);
    }

    pub fn on_get_account_total_balance_failed(&mut self, pool: &mut ValidatorPool) {
        self.post_execution(pool);
    }
    
    /// Due to shares calculation and rounding of staking pool contract,
    /// the amount of staked and unstaked balance might be a little bit
    /// different than we requested.
    /// This method is to sync the actual numbers with the validator.
    ///
    /// Params:
    /// - pool: validator pool
    /// - post_action: sync balance is called after stake or unstake
    pub fn sync_account_balance(&mut self, pool: &mut ValidatorPool, post_action: bool) -> Promise {
        if post_action {
            require!(self.executing, ERR_VALIDATOR_SYNC_BALANCE_NOT_EXPECTED);
        } else {
            self.pre_execution(pool);
        }

        ext_staking_pool::ext(self.account_id.clone())
            .with_static_gas(GAS_EXT_GET_ACCOUNT)
            .get_account(env::current_account_id())
    }

    pub fn on_sync_account_balance_success(
        &mut self,
        pool: &mut ValidatorPool,
        staked_balance: u128,
        unstaked_balance: u128,
    ) {
        self.post_execution(pool);

        // sync base stake amount
        let new_total_balance = staked_balance + unstaked_balance;
        self.sync_base_stake_amount(pool, new_total_balance);

        // update balance
        self.staked_amount = staked_balance;
        self.unstaked_amount = unstaked_balance;

        pool.save_validator(self);
    }

    pub fn on_sync_account_balance_failed(&mut self, pool: &mut ValidatorPool) {
        self.post_execution(pool);
    }

    pub fn withdraw(&mut self, pool: &mut ValidatorPool, amount: u128) -> Promise {
        self.pre_execution(pool);

        require!(
            self.unstaked_amount >= amount,
            ERR_NO_ENOUGH_WITHDRAW_BALANCE
        );
        require!(!self.pending_release(), ERR_VALIDATOR_WITHDRAW_WHEN_LOCKED);

        self.unstaked_amount -= amount;
        pool.save_validator(self);

        ext_staking_pool::ext(self.account_id.clone())
            .with_static_gas(GAS_EXT_WITHDRAW)
            .withdraw(amount.into())
    }

    pub fn on_withdraw_success(&mut self, pool: &mut ValidatorPool) {
        self.post_execution(pool);
    }

    pub fn on_withdraw_failed(&mut self, pool: &mut ValidatorPool, amount: u128) {
        self.post_execution(pool);

        self.unstaked_amount += amount;
        pool.save_validator(self);
    }

    pub fn set_draining(&mut self, pool: &mut ValidatorPool, draining: bool) {
        self.draining = draining;
        pool.save_validator(self);
    }

    fn sync_base_stake_amount(&mut self, pool: &mut ValidatorPool, new_total_balance: u128) {
        let old_total_balance = self.staked_amount + self.unstaked_amount;
        // If no balance, or no base stake amount set, no need to update base stake amount
        if old_total_balance != 0 && self.base_stake_amount != 0 {
            let old_base_stake_amount = self.base_stake_amount;
            self.base_stake_amount = (U256::from(old_base_stake_amount)
                * U256::from(new_total_balance)
                / U256::from(old_total_balance))
            .as_u128();
            pool.total_base_stake_amount =
                pool.total_base_stake_amount + self.base_stake_amount - old_base_stake_amount;
        }
    }

    fn pre_execution(&mut self, pool: &mut ValidatorPool) {
        require!(!self.executing, ERR_VALIDATOR_ALREADY_EXECUTING_ACTION);
        self.executing = true;
        pool.save_validator(self);
    }

    fn post_execution(&mut self, pool: &mut ValidatorPool) {
        self.executing = false;
        pool.save_validator(self);
    }
}

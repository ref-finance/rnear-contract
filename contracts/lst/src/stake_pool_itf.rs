use crate::*;

/// poolv1 interfaces
/// https://github.com/near/core-contracts/blob/master/staking-pool/src/lib.rs
/// this contract could be taken by users as a regular validator poolv1 contract

#[near]
impl Contract {
    /// Returns the unstaked balance of the given account.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128 {
        self.get_account(account_id).unstaked_balance
    }

    /// Returns the staked balance of the given account.
    /// NOTE: This is computed from the amount of "stake" shares the given account has and the
    /// current amount of total staked balance and total stake shares on the account.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128 {
        self.get_account(account_id).staked_balance
    }

    /// Returns the total balance of the given account (including staked and unstaked balances).
    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128 {
        let account = self.get_account(account_id);
        (account.unstaked_balance.0 + account.staked_balance.0).into()
    }

    /// Returns `true` if the given account can withdraw tokens in the current epoch.
    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        self.get_account(account_id).can_withdraw
    }

    /// Returns the total staking balance.
    pub fn get_total_staked_balance(&self) -> U128 {
        self.data().total_staked_near_amount.into()
    }

    /// Returns account ID of the staking pool owner.
    pub fn get_owner_id(&self) -> AccountId {
        env::current_account_id()
    }

    pub fn get_reward_fee_fraction(&self) -> RewardFeeFraction {
        RewardFeeFraction {
            numerator: self.data().beneficiaries.values().sum(),
            denominator: 10000,
        }
    }

    /// Returns the staking public key
    pub fn get_staking_key(&self) -> PublicKey {
        panic!("no need to specify public key for liquid staking pool");
    }

    /// Returns true if the staking is paused
    pub fn is_paused(&self) -> bool {
        self.pa_is_paused("ALL".to_string())
    }

    /// Returns human readable representation of the account for the given account ID.
    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        let account = self.internal_get_account(&account_id);
        let stake_shares = self.data().token.ft_balance_of(account_id.clone());
        HumanReadableAccount {
            account_id,
            unstaked_balance: account.unstaked.into(),
            staked_balance: self
                .staked_amount_from_num_shares_rounded_down(stake_shares.into())
                .into(),
            can_withdraw: account.unstaked_available_epoch_height <= get_epoch_height(),
        }
    }

    /// Returns the number of accounts that have positive balance on this staking pool.
    pub fn get_number_of_accounts(&self) -> u64 {
        self.data().accounts.len() as u64
    }

    /// Returns the list of accounts
    pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount> {
        let skip_n = from_index as usize;
        let take_n = limit as usize;

        self.data()
            .accounts
            .keys()
            .skip(skip_n)
            .take(take_n)
            .map(|k| self.get_account(k.clone()))
            .collect()
    }

    /// Please notice ping() is not available for liquid staking.
    /// Keep here for interface consistency.
    pub fn ping(&mut self) {}

    /// Deposits the attached amount into the inner account of the predecessor.
    /// will charge standard FT storage fee if needed.
    #[payable]
    pub fn deposit(&mut self) {
        let amount = env::attached_deposit().as_yoctonear();
        let account_id = env::predecessor_account_id();
        let storage_used = if self.storage_balance_of(account_id.clone()).is_none() {
            self.data_mut()
                .accounts
                .insert(account_id.clone(), Account::default());
            self.data_mut().token.internal_register_account(&account_id);
            self.storage_balance_bounds().min.as_yoctonear()
        } else {
            0
        };
        self.internal_deposit(amount - storage_used);
    }

    /// Deposits the attached amount into the inner account of the predecessor and stakes it.
    /// will charge standard FT storage fee if needed.
    /// Returns the received LST amount
    #[payable]
    pub fn deposit_and_stake(&mut self) -> U128 {
        let amount = env::attached_deposit().as_yoctonear();
        let account_id = env::predecessor_account_id();
        let storage_used = if self.storage_balance_of(account_id.clone()).is_none() {
            self.data_mut()
                .accounts
                .insert(account_id.clone(), Account::default());
            self.data_mut().token.internal_register_account(&account_id);
            self.storage_balance_bounds().min.as_yoctonear()
        } else {
            // log!("already registered.");
            0
        };
        self.internal_deposit(amount - storage_used);
        self.internal_stake(amount - storage_used).into()
    }

    /// Withdraws the entire unstaked balance from the predecessor account.
    /// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
    pub fn withdraw_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_withdraw(account.unstaked);
    }

    /// Withdraws the non staked balance for given account.
    /// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
    pub fn withdraw(&mut self, amount: U128) {
        self.internal_withdraw(amount.into());
    }

    /// Stakes all available unstaked balance from the inner account of the predecessor.
    pub fn stake_all(&mut self) -> U128 {
        let account_id = env::predecessor_account_id();
        let account = self.internal_get_account(&account_id);
        self.internal_stake(account.unstaked).into()
    }

    /// Stakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough unstaked balance.
    pub fn stake(&mut self, amount: U128) -> U128 {
        self.internal_stake(amount.into()).into()
    }

    /// Unstakes all staked balance from the inner account of the predecessor.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake_all(&mut self) {
        let stake_shares = self
            .data()
            .token
            .ft_balance_of(env::predecessor_account_id());
        let amount = self.staked_amount_from_num_shares_rounded_down(stake_shares.into());
        self.internal_unstake(amount);
    }

    /// Unstakes the given amount from the inner account of the predecessor.
    /// The inner account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in four epochs.
    pub fn unstake(&mut self, amount: U128) {
        self.internal_unstake(amount.into());
    }
}

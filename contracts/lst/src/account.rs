use crate::*;

#[near(serializers = [borsh])]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Account {
    /// The unstaked balance. It represents the amount the account has on this contract that
    /// can either be staked or withdrawn.
    pub unstaked: u128,
    /// The amount of "stake" shares. Every stake share corresponds to the amount of staked balance.
    /// NOTE: The number of shares should always be less or equal than the amount of staked balance.
    /// This means the price of stake share should always be at least `1`.
    /// The price of stake share can be computed as `total_staked_balance` / `total_share_amount`.
    // pub stake_shares: ShareBalance,

    /// The minimum epoch height when the withdrawn is allowed.
    /// This changes after unstaking action, because the amount is still locked for 3 epochs.
    pub unstaked_available_epoch_height: EpochHeight,
}

#[near(serializers = [json])]
pub struct HumanReadableAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: U128,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

#[near(serializers = [json])]
pub struct AccountDetailsView {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: U128,
    /// The minimum epoch height when the withdrawn is allowed.
    /// This changes after unstaking action, because the amount is still locked for 3 epochs.
    pub unstaked_available_epoch_height: EpochHeight,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

impl Contract {
    pub(crate) fn internal_get_account(&self, account_id: &AccountId) -> Account {
        // self.data().accounts.get(account_id).clone().unwrap_or_default()
        if let Some(acc) = self.data().accounts.get(account_id) {
            acc.clone()
        } else {
            Account::default()
        }
    }

    pub(crate) fn internal_save_account(&mut self, account_id: &AccountId, account: &Account) {
        self.data_mut().accounts.insert(account_id.clone(), account.clone());
    }

    #[pause]
    pub(crate) fn internal_deposit(&mut self, amount: u128) {
        require!(amount > 0, ERR_NON_POSITIVE_DEPOSIT_AMOUNT);

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        account.unstaked += amount;
        self.internal_save_account(&account_id, &account);

        Event::Deposit {
            account_id: &account_id,
            amount: &U128(amount),
            new_unstaked_balance: &U128(account.unstaked),
        }
        .emit();
    }

    pub(crate) fn assert_can_withdraw(&self, account_id: &AccountId, amount: u128) {
        require!(amount > 0, ERR_NON_POSITIVE_WITHDRAWAL_AMOUNT);

        let account = self.internal_get_account(account_id);
        require!(account.unstaked >= amount, ERR_NO_ENOUGH_UNSTAKED_BALANCE_TO_WITHDRAW);
        require!(
            account.unstaked_available_epoch_height <= get_epoch_height(),
            ERR_UNSTAKED_BALANCE_NOT_AVAILABLE
        );
        require!(self.available_near() >= amount, ERR_NO_ENOUGH_CONTRACT_BALANCE);
    }

    #[pause]
    pub(crate) fn internal_withdraw(&mut self, amount: u128) {
        let account_id = env::predecessor_account_id();
        self.assert_can_withdraw(&account_id, amount);

        let mut account = self.internal_get_account(&account_id);
        account.unstaked -= amount;
        self.internal_save_account(&account_id, &account);

        Event::Withdraw {
            account_id: &account_id,
            amount: &U128(amount),
            new_unstaked_balance: &U128(account.unstaked),
        }
        .emit();
        Promise::new(account_id).transfer(NearToken::from_yoctonear(amount));
    }

    #[pause]
    pub(crate) fn internal_stake(&mut self, amount: u128) -> ShareBalance {
        require!(amount > 0, ERR_NON_POSITIVE_STAKING_AMOUNT);

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        // Calculate the number of "stake" shares that the account will receive for staking the
        // given amount.
        let num_shares = self.num_shares_from_staked_amount_rounded_down(amount);
        require!(num_shares > 0, ERR_NON_POSITIVE_CALCULATED_STAKING_SHARE);

        require!(account.unstaked >= amount, ERR_NO_ENOUGH_UNSTAKED_BALANCE);
        account.unstaked -= amount;
        self.mint_lst(&account_id, num_shares, Some("stake"));
        self.internal_save_account(&account_id, &account);
        self.data_mut().total_staked_near_amount += amount;
        // Increase requested stake amount within the current epoch
        self.data_mut().epoch_requested_stake_amount += amount;

        Event::Stake {
            account_id: &account_id,
            staked_amount: &U128(amount),
            minted_stake_shares: &U128(num_shares),
            new_unstaked_balance: &U128(account.unstaked),
            new_stake_shares: &U128(self.data().token.accounts.get(&account_id).unwrap_or(0)),
        }
        .emit();

        log!(
            "Contract total staked balance is {}. Total number of shares {}",
            self.data().total_staked_near_amount,
            self.data().token.total_supply
        );

        num_shares
    }

    #[pause]
    pub(crate) fn internal_unstake(&mut self, amount: u128) {
        require!(amount > 0, ERR_NON_POSITIVE_UNSTAKING_AMOUNT);

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        require!(self.data().total_staked_near_amount > 0, ERR_CONTRACT_NO_STAKED_BALANCE);
        // Calculate the number of shares required to unstake the given amount.
        // NOTE: The number of shares the account will pay is rounded up.
        let num_shares = self.num_shares_from_staked_amount_rounded_up(amount);
        require!(num_shares > 0, ERR_NON_POSITIVE_CALCULATED_UNSTAKING_SHARE);

        self.burn_lst(&account_id, num_shares, Some("unstake"));

        account.unstaked += amount;
        account.unstaked_available_epoch_height = get_epoch_height() + self.data().validator_pool.get_num_epoch_to_unstake(amount);
        if self.data().last_settlement_epoch == get_epoch_height() {
            // The unstake request is received after epoch_cleanup
            // so actual unstake will happen in the next epoch,
            // which will put withdraw off for one more epoch.
            account.unstaked_available_epoch_height += 1;
        }

        self.internal_save_account(&account_id, &account);

        self.data_mut().total_staked_near_amount -= amount;

        // Increase requested unstake amount within the current epoch
        self.data_mut().epoch_requested_unstake_amount += amount;

        Event::Unstake {
            account_id: &account_id,
            unstaked_amount: &U128(amount),
            burnt_stake_shares: &U128(num_shares),
            new_unstaked_balance: &U128(account.unstaked),
            new_stake_shares: &U128(self.data().token.accounts.get(&account_id).unwrap_or(0)),
            unstaked_available_epoch_height: account.unstaked_available_epoch_height,
        }
        .emit();

        log!(
            "Contract total staked balance is {}. Total number of shares {}",
            self.data().total_staked_near_amount,
            self.data().token.total_supply
        );
    }
}

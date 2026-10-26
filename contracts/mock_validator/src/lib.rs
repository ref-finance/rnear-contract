use near_sdk::json_types::U128;
use near_sdk::store::IterableMap;
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, Promise};

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

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct Contract {
    deposits: IterableMap<AccountId, u128>,
    staked: IterableMap<AccountId, u128>,
    /// for testing purpose, simulates contract panic
    panic: bool,
    get_account_fail: bool,

    staked_delta: u128,
    unstaked_delta: u128,
    pending_reward: u128,
}

#[near]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Self {
            deposits: IterableMap::new(b"d"),
            staked: IterableMap::new(b"s"),
            panic: false,
            get_account_fail: false,
            staked_delta: 0,
            unstaked_delta: 0,
            pending_reward: 0,
        }
    }
}

#[near]
impl Contract {
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128 {
        require!(!self.panic, "Test Panic!");
        U128::from(self.internal_get_staked(&account_id))
    }

    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128 {
        require!(!self.panic, "Test Panic!");
        U128::from(self.internal_get_unstaked_deposit(&account_id))
    }

    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128 {
        require!(!self.panic, "Test Panic!");
        U128::from(
            self.internal_get_unstaked_deposit(&account_id) + self.internal_get_staked(&account_id),
        )
    }

    pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount {
        require!(!self.panic, "Test Panic!");
        require!(
            !self.get_account_fail,
            "get_account() failed, for testing purpose",
        );
        HumanReadableAccount {
            account_id: account_id.clone(),
            staked_balance: U128::from(self.internal_get_staked(&account_id)),
            unstaked_balance: U128::from(self.internal_get_unstaked_deposit(&account_id)),
            can_withdraw: true,
        }
    }

    #[payable]
    pub fn deposit(&mut self) {
        require!(!self.panic, "Test Panic!");
        self.internal_deposit();
    }

    #[payable]
    pub fn deposit_and_stake(&mut self) {
        require!(!self.panic, "Test Panic!");
        self.apply_pending_reward();
        let amount = self.internal_deposit();
        self.internal_stake(amount);
    }

    pub fn withdraw(&mut self, amount: U128) {
        require!(!self.panic, "Test Panic!");
        let account_id = env::predecessor_account_id();
        self.internal_withdraw(&account_id, amount.0);
    }

    pub fn withdraw_all(&mut self) {
        require!(!self.panic, "Test Panic!");
        let account_id = env::predecessor_account_id();
        let unstaked = self.internal_get_unstaked_deposit(&account_id);
        self.internal_withdraw(&account_id, unstaked);
    }

    pub fn stake(&mut self, amount: U128) {
        require!(!self.panic, "Test Panic!");
        self.apply_pending_reward();
        self.internal_stake(amount.0)
    }

    pub fn unstake(&mut self, amount: U128) {
        require!(!self.panic, "Test Panic!");
        self.apply_pending_reward();
        self.internal_unstake(amount.0);
    }

    pub fn unstake_all(&mut self) {
        require!(!self.panic, "Test Panic!");
        self.apply_pending_reward();
        let account_id = env::predecessor_account_id();
        let staked_amount = self.internal_get_staked(&account_id);
        self.internal_unstake(staked_amount);
    }
}

#[near]
impl Contract {
    /// manually generate some reward for the caller,
    /// for testing purpose only
    pub fn add_reward(&mut self, amount: U128) {
        let account_id = env::predecessor_account_id();
        self.add_reward_for(amount, account_id);
    }

    pub fn add_reward_for(&mut self, amount: U128, account_id: AccountId) {
        let staked_amount = self.internal_get_staked(&account_id);
        // disable assert of `staked amount > 0` to test one special case that rewards are
        // received when call `unstake()` on staking pool, which triggers `internal_ping()`
        // assert!(staked_amount > 0);

        let new_amount = staked_amount + amount.0;
        self.staked.insert(account_id, new_amount);
    }

    pub fn set_panic(&mut self, panic: bool) {
        self.panic = panic;
    }

    pub fn set_get_account_fail(&mut self, value: bool) {
        self.get_account_fail = value;
    }

    pub fn set_balance_delta(&mut self, staked_delta: U128, unstaked_delta: U128) {
        self.staked_delta = staked_delta.0;
        self.unstaked_delta = unstaked_delta.0;
    }

    pub fn set_pending_reward(&mut self, amount: U128) {
        self.pending_reward = amount.0;
    }
}

impl Contract {
    fn apply_pending_reward(&mut self) {
        let reward = std::mem::take(&mut self.pending_reward);
        if reward > 0 {
            let account_id = env::predecessor_account_id();
            let staked = self.internal_get_staked(&account_id);
            self.staked.insert(account_id, staked + reward);
        }
    }

    fn internal_deposit(&mut self) -> u128 {
        let account_id = env::predecessor_account_id();
        let amount = env::attached_deposit().as_yoctonear();
        assert!(amount > 0);

        let current_deposit = self.internal_get_unstaked_deposit(&account_id);
        let new_deposit = current_deposit + amount;

        self.deposits.insert(account_id, new_deposit);
        amount
    }

    fn internal_stake(&mut self, amount: u128) {
        let account_id = env::predecessor_account_id();
        let unstaked_deposit = self.internal_get_unstaked_deposit(&account_id);
        assert!(unstaked_deposit >= amount);

        let new_deposit = unstaked_deposit - amount + self.unstaked_delta;
        let new_staked =
            (self.internal_get_staked(&account_id) + amount).saturating_sub(self.staked_delta);

        self.deposits.insert(account_id.clone(), new_deposit);
        self.staked.insert(account_id.clone(), new_staked);
    }

    fn internal_unstake(&mut self, amount: u128) {
        let account_id = env::predecessor_account_id();
        let staked = self.internal_get_staked(&account_id);
        assert!(staked >= amount);

        let unstaked_deposit = self.internal_get_unstaked_deposit(&account_id);
        let new_deposit = unstaked_deposit + amount + self.unstaked_delta;
        let new_staked = (staked - amount).saturating_sub(self.staked_delta);

        self.deposits.insert(account_id.clone(), new_deposit);
        self.staked.insert(account_id.clone(), new_staked);
    }

    fn internal_withdraw(&mut self, account_id: &AccountId, amount: u128) {
        let unstaked_amount = self.internal_get_unstaked_deposit(account_id);
        assert!(unstaked_amount >= amount);

        let new_unstaked = unstaked_amount - amount;
        self.deposits.insert(account_id.clone(), new_unstaked);

        Promise::new(account_id.clone()).transfer(NearToken::from_yoctonear(amount));
    }

    fn internal_get_unstaked_deposit(&self, account_id: &AccountId) -> u128 {
        *self.deposits.get(account_id).unwrap_or(&0)
    }

    fn internal_get_staked(&self, account_id: &AccountId) -> u128 {
        *self.staked.get(account_id).unwrap_or(&0)
    }
}

use crate::*;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::serde_json;
use near_sdk::PromiseResult;

#[ext_contract(ext_wnear)]
#[allow(dead_code)]
trait WnearContract {
    fn near_withdraw(&mut self, amount: U128);
    fn near_deposit(&mut self);
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>, msg: String) -> U128;
}

#[ext_contract(ext_platform_receiver)]
#[allow(dead_code)]
trait PlatformReceiver {
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> U128;
}

pub(crate) const MAX_MESSAGE_BYTES: usize = 2048;
const MAX_REQUEST_KEYS_PER_PLATFORM: u32 = 256;
const MAX_REQUEST_KEYS_GLOBAL: u32 = 4096;
pub(crate) const MAX_INFLIGHT_PER_PLATFORM: u32 = 16;
const MAX_INFLIGHT_GLOBAL: u32 = 256;
pub(crate) const MAX_TRANCHES_PER_REQUEST: usize = 16;
const MIN_STAKE_AMOUNT: u128 = 1;
const MIN_EMERGENCY_STORAGE_RESERVE: u128 = 5 * ONE_NEAR;
// near-sdk ext builders default to GasWeight(1); every fixed-budget call below sets it to zero.
const GAS_WNEAR_WITHDRAW: Gas = Gas::from_tgas(10);
// Must also schedule the platform receiver (at most 40 Tgas) and resolver (40 Tgas).
const GAS_ON_WNEAR_UNWRAPPED: Gas = Gas::from_tgas(100);
const GAS_STAKE_RESOLVER: Gas = Gas::from_tgas(40);
const GAS_WNEAR_WRAP: Gas = Gas::from_tgas(10);
// The recovery wrap callback must schedule ft_transfer_call (at most 60 + 30 Tgas) and its 25 Tgas resolver.
const GAS_ON_STAKE_RECOVERY_WRAP: Gas = Gas::from_tgas(135);
const GAS_ON_STAKE_RECOVERY_TRANSFER: Gas = Gas::from_tgas(25);
// Schedules ft_transfer_call (at most 60 + 30 Tgas) and its 25 Tgas callback;
// the remaining 20 Tgas is for this callback's own work.
const GAS_ON_NEAR_DEPOSIT: Gas = Gas::from_tgas(135);
const GAS_ON_WNEAR_TRANSFER: Gas = Gas::from_tgas(25);

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct StakeMessage {
    pub receiver_id: AccountId,
    pub msg: String,
    pub memo: Option<String>,
    #[serde(default)]
    pub is_storage_deposit: bool,
    pub min_gas: Option<Gas>,
    pub refund_message: UnstakeMessage,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, PartialEq, Eq)]
pub struct UnstakeMessage {
    pub receiver_id: AccountId,
    pub withdraw_tokens: WithdrawTokens,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WithdrawTokens {
    Wnear {
        #[serde(default)]
        is_storage_deposit: bool,
        msg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        memo: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_gas: Option<Gas>,
    },
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlatformUserStatus {
    Active,
    Draining,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct PlatformUser {
    pub status: PlatformUserStatus,
    pub refund_account_id: AccountId,
    pub sponsored_ft_registration: bool,
    pub storage_deposit_total: u128,
    pub storage_used_bytes: u64,
    pub storage_reserved_bytes: u64,
    pub storage_refund_pending: u128,
    pub inflight_storage_operations: u32,
    pub active_request_keys: u32,
}

#[near(serializers = [borsh])]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlatformRequestKey {
    pub platform_id: AccountId,
    pub request_key: CryptoHash,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct WithdrawalRequest {
    pub message: UnstakeMessage,
    pub tranches: Vec<TrancheLock>,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeRecoveryAsset {
    Near,
    Wnear,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeLostFoundStatus {
    Processing,
    Near,
    Wnear,
    Wrapping,
    Sending,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct StakeLostFound {
    pub platform_id: AccountId,
    pub amount: u128,
    pub refund_message: UnstakeMessage,
    pub reserved_bytes: u64,
    pub created_block_height: u64,
    pub status: StakeLostFoundStatus,
}

#[near(serializers = [json])]
pub struct StakeLostFoundView {
    pub attempt_id: u64,
    pub platform_id: AccountId,
    pub amount: U128,
    pub refund_message: UnstakeMessage,
    pub created_block_height: u64,
    pub status: StakeLostFoundStatus,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct WithdrawalTranche {
    pub amount: u128,
    pub available_epoch_height: EpochHeight,
    pub wnear_residual: u128,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub enum TrancheLock {
    Unlocked(WithdrawalTranche),
    Wrapping { tranche: WithdrawalTranche, amount_to_wrap: u128 },
    WnearSending { tranche: WithdrawalTranche, amount_to_send: u128 },
}

impl TrancheLock {
    pub(crate) fn unlocked(&self) -> Option<&WithdrawalTranche> {
        match self {
            Self::Unlocked(tranche) => Some(tranche),
            _ => None,
        }
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.unlocked().is_none()
    }
}

#[near(serializers = [json])]
pub struct PlatformUserView {
    pub platform_id: AccountId,
    pub status: PlatformUserStatus,
    pub refund_account_id: AccountId,
    pub sponsored_ft_registration: bool,
    pub storage_deposit_total: U128,
    pub storage_used_bytes: u64,
    pub storage_reserved_bytes: u64,
    pub storage_refund_pending: U128,
    pub inflight_storage_operations: u32,
    pub active_request_keys: u32,
    pub required_storage_balance: U128,
    pub storage_overdraft: U128,
}

#[near(serializers = [json])]
pub struct WithdrawalTrancheView {
    pub amount: U128,
    pub available_epoch_height: EpochHeight,
    pub wnear_residual: U128,
    pub matured: bool,
    pub locked: bool,
}

#[near(serializers = [json])]
pub struct WithdrawalRequestView {
    pub platform_id: AccountId,
    pub request_key: Base58CryptoHash,
    pub tranches: Vec<WithdrawalTrancheView>,
}

impl PlatformUser {
    fn required_storage_balance(&self) -> u128 {
        (self.storage_used_bytes as u128)
            .checked_add(self.storage_reserved_bytes as u128)
            .and_then(|bytes| bytes.checked_mul(env::storage_byte_cost().as_yoctonear()))
            .expect("Platform storage cost overflow")
    }

    fn overdraft(&self) -> u128 {
        self.required_storage_balance().saturating_sub(self.storage_deposit_total)
    }
}

impl Contract {
    fn configured_wnear_id(&self) -> AccountId {
        self.data().wnear_id.clone().expect("wNEAR is not configured")
    }

    pub(crate) fn platform_user(&self, platform_id: &AccountId) -> PlatformUser {
        self.data().platform_users.get(platform_id).cloned().expect("Platform user is not registered")
    }

    fn save_platform_user(&mut self, platform_id: &AccountId, user: PlatformUser) {
        self.data_mut().platform_users.insert(platform_id.clone(), user);
        self.data_mut().platform_users.flush();
    }

    fn require_active_platform(&self, platform_id: &AccountId) -> PlatformUser {
        let user = self.platform_user(platform_id);
        require!(user.status == PlatformUserStatus::Active, "Platform user is draining");
        require!(user.overdraft() == 0, "Platform storage is overdrawn");
        user
    }

    fn validate_unstake_message(message: &UnstakeMessage) {
        let WithdrawTokens::Wnear {
            is_storage_deposit,
            msg,
            memo,
            min_gas,
        } = &message.withdraw_tokens;
        require!(!is_storage_deposit, "Storage deposit is not supported");
        require!(!msg.is_empty(), "wNEAR message is required");
        require!(msg.len() <= MAX_MESSAGE_BYTES, "Message is too long");
        if let Some(memo) = memo {
            require!(memo.len() <= MAX_MESSAGE_BYTES, "Memo is too long");
        }
        if let Some(gas) = min_gas {
            // Allow at least 30 Tgas for the receiver; cap it at 60 so 60 + 30 + 25 fits the 135 Tgas wrapping callback.
            require!(
                gas.as_gas() >= Gas::from_tgas(30).as_gas() && gas.as_gas() <= Gas::from_tgas(60).as_gas(),
                "Invalid receiver gas"
            );
        }
    }

    pub(crate) fn request_key(message: &UnstakeMessage) -> CryptoHash {
        env::keccak256_array(&near_sdk::borsh::to_vec(message).expect("Invalid request message"))
    }

    pub(crate) fn checked_reservation(message: &UnstakeMessage) -> u64 {
        let bytes = near_sdk::borsh::to_vec(message).expect("Invalid request message").len();
        require!(bytes <= MAX_MESSAGE_BYTES * 2 + 256, "Request message is too long");
        u64::try_from(bytes.checked_mul(2).and_then(|n| n.checked_add(512)).unwrap()).unwrap()
    }

    pub(crate) fn reserve_platform_storage(&mut self, platform_id: &AccountId, bytes: u64) {
        let mut user = self.require_active_platform(platform_id);
        require!(user.inflight_storage_operations < MAX_INFLIGHT_PER_PLATFORM, "Platform inflight limit");
        require!(self.data().total_inflight_storage_operations < MAX_INFLIGHT_GLOBAL, "Global inflight limit");
        user.storage_reserved_bytes = user.storage_reserved_bytes.checked_add(bytes).expect("Storage reservation overflow");
        user.inflight_storage_operations += 1;
        require!(
            user.required_storage_balance() <= user.storage_deposit_total,
            "Insufficient platform storage credit"
        );
        self.data_mut().total_inflight_storage_operations += 1;
        self.save_platform_user(platform_id, user);
    }

    pub(crate) fn release_platform_storage(&mut self, platform_id: &AccountId, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let mut user = self.platform_user(platform_id);
        user.storage_reserved_bytes = user.storage_reserved_bytes.checked_sub(bytes).expect("Missing storage reservation");
        user.inflight_storage_operations = user.inflight_storage_operations.checked_sub(1).expect("Missing inflight operation");
        self.data_mut().total_inflight_storage_operations = self
            .data()
            .total_inflight_storage_operations
            .checked_sub(1)
            .expect("Missing global inflight operation");
        self.save_platform_user(platform_id, user);
    }

    pub(crate) fn save_request(&mut self, key: &PlatformRequestKey, request: Option<WithdrawalRequest>) {
        let existed = self.data().withdrawal_requests.get(key).is_some();
        let created = request.is_some();
        let before = env::storage_usage();
        match request {
            Some(request) => {
                self.data_mut().withdrawal_requests.insert(key.clone(), request);
            }
            None => {
                self.data_mut().withdrawal_requests.remove(key);
            }
        }
        self.data_mut().withdrawal_requests.flush();
        let after = env::storage_usage();
        let mut user = self.platform_user(&key.platform_id);
        if after >= before {
            user.storage_used_bytes = user.storage_used_bytes.checked_add(after - before).expect("Storage usage overflow");
        } else {
            user.storage_used_bytes = user.storage_used_bytes.checked_sub(before - after).expect("Storage usage underflow");
        }
        if !existed && created {
            user.active_request_keys = user.active_request_keys.checked_add(1).expect("Key count overflow");
            self.data_mut().total_active_request_keys = self.data().total_active_request_keys.checked_add(1).expect("Global key count overflow");
        } else if existed && !created {
            user.active_request_keys = user.active_request_keys.checked_sub(1).expect("Key count underflow");
            self.data_mut().total_active_request_keys = self.data().total_active_request_keys.checked_sub(1).expect("Global key count underflow");
        }
        self.save_platform_user(&key.platform_id, user);
    }

    pub(crate) fn save_stake_lostfound(&mut self, attempt_id: u64, entry: Option<StakeLostFound>, platform_id: &AccountId) {
        let before = env::storage_usage();
        if let Some(entry) = entry {
            self.data_mut().stake_lostfound.insert(attempt_id, entry);
        } else {
            self.data_mut().stake_lostfound.remove(&attempt_id);
        }
        self.data_mut().stake_lostfound.flush();
        let after = env::storage_usage();
        let mut user = self.platform_user(platform_id);
        if after >= before {
            user.storage_used_bytes = user.storage_used_bytes.checked_add(after - before).expect("Storage usage overflow");
        } else {
            user.storage_used_bytes = user.storage_used_bytes.checked_sub(before - after).expect("Storage usage underflow");
        }
        self.save_platform_user(platform_id, user);
    }

    fn remove_stake_lostfound(&mut self, attempt_id: u64, entry: &StakeLostFound) {
        self.save_stake_lostfound(attempt_id, None, &entry.platform_id);
        if matches!(
            entry.status,
            StakeLostFoundStatus::Processing | StakeLostFoundStatus::Near | StakeLostFoundStatus::Wrapping
        ) {
            self.data_mut().stake_lostfound_near_reserved = self
                .data()
                .stake_lostfound_near_reserved
                .checked_sub(entry.amount)
                .expect("Lost-found NEAR reserve underflow");
        }
    }

    fn send_stake_recovery_wnear(&self, attempt_id: u64, entry: &StakeLostFound) -> Promise {
        let WithdrawTokens::Wnear { msg, memo, min_gas, .. } = &entry.refund_message.withdraw_tokens;
        let receiver_gas = min_gas.unwrap_or(Gas::from_tgas(35));
        ext_wnear::ext(self.configured_wnear_id())
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(receiver_gas.checked_add(Gas::from_tgas(30)).expect("Gas overflow"))
            .with_unused_gas_weight(0)
            .ft_transfer_call(entry.refund_message.receiver_id.clone(), U128(entry.amount), memo.clone(), msg.clone())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_ON_STAKE_RECOVERY_TRANSFER)
                    .with_unused_gas_weight(0)
                    .on_stake_recovery_transfer(attempt_id),
            )
    }

    fn settle_stake_recovery_transfer(&mut self, attempt_id: u64, consumed: u128) {
        let mut entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Sending, "Stake recovery is not sending");
        require!(consumed <= entry.amount, "Recovered amount exceeds send amount");
        let remaining = entry.amount - consumed;
        if remaining == 0 {
            self.remove_stake_lostfound(attempt_id, &entry);
            self.release_platform_storage(&entry.platform_id, entry.reserved_bytes);
        } else {
            entry.amount = remaining;
            entry.status = StakeLostFoundStatus::Wnear;
            self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
        }
        Event::StakeRecoveryDelivered {
            attempt_id,
            platform_id: &entry.platform_id,
            consumed: U128(consumed),
            remaining: U128(remaining),
        }
        .emit();
    }

    fn report_platform_overdraft(&self, platform_id: &AccountId) {
        let amount = self.platform_user(platform_id).overdraft();
        if amount > 0 {
            Event::PlatformUserStorageOverdrawn {
                platform_id,
                amount: U128(amount),
            }
            .emit();
        }
    }

    // None starts a fresh unstake with admission checks; Some consumes storage
    // reserved before the asynchronous stake delivery and releases it after enqueueing.
    pub(crate) fn internal_unstake_shares_to_request(
        &mut self,
        platform_id: &AccountId,
        shares: u128,
        message: &UnstakeMessage,
        reserved_bytes: Option<u64>,
    ) -> CryptoHash {
        require!(shares > 0, "Unstake shares must be positive");
        self.configured_wnear_id();
        Self::validate_unstake_message(message);
        let key_hash = Self::request_key(message);
        let key = PlatformRequestKey {
            platform_id: platform_id.clone(),
            request_key: key_hash,
        };
        let existing = self.data().withdrawal_requests.get(&key).cloned();
        if let Some(existing) = &existing {
            require!(
                Self::request_key(&existing.message) == key_hash && existing.message == *message,
                "Withdrawal route mismatch"
            );
        }
        let user = if reserved_bytes.is_none() {
            self.require_active_platform(platform_id)
        } else {
            self.platform_user(platform_id)
        };
        if existing.is_none() && reserved_bytes.is_none() {
            require!(
                user.active_request_keys.saturating_add(user.inflight_storage_operations) < MAX_REQUEST_KEYS_PER_PLATFORM,
                "Platform request limit"
            );
            require!(
                self.data()
                    .total_active_request_keys
                    .saturating_add(self.data().total_inflight_storage_operations)
                    < MAX_REQUEST_KEYS_GLOBAL,
                "Global request limit"
            );
        }
        let admission_bytes = Self::checked_reservation(message);
        if let Some(reserved) = reserved_bytes {
            require!(
                user.storage_reserved_bytes >= reserved && reserved >= admission_bytes,
                "Missing withdrawal storage reservation"
            );
        } else {
            let required = (user.storage_used_bytes as u128)
                .checked_add(user.storage_reserved_bytes as u128)
                .and_then(|n| n.checked_add(admission_bytes as u128))
                .and_then(|n| n.checked_mul(env::storage_byte_cost().as_yoctonear()))
                .expect("Storage credit calculation overflow");
            require!(required <= user.storage_deposit_total, "Insufficient platform storage credit");
        }
        let contract_id = env::current_account_id();
        require!(
            self.data().token.accounts.get(&contract_id).unwrap_or(0) >= shares,
            "Insufficient received shares"
        );
        let amount = self.staked_amount_from_num_shares_rounded_down(shares);
        require!(amount > 0 && amount <= self.data().total_staked_near_amount, "Unstake amount rounds to zero");
        let mut available_epoch_height = get_epoch_height()
            .checked_add(self.data().validator_pool.get_num_epoch_to_unstake(amount))
            .expect("Withdrawal epoch overflow");
        if self.data().last_settlement_epoch == get_epoch_height() {
            available_epoch_height = available_epoch_height.checked_add(1).expect("Withdrawal epoch overflow");
        }
        let mut request = existing.unwrap_or(WithdrawalRequest {
            message: message.clone(),
            tranches: Vec::new(),
        });
        let merge_index = request.tranches.iter().position(|lock| {
            lock.unlocked()
                .map(|tranche| tranche.available_epoch_height == available_epoch_height)
                .unwrap_or(false)
        });
        if let Some(index) = merge_index {
            if let TrancheLock::Unlocked(tranche) = &mut request.tranches[index] {
                tranche.amount = tranche.amount.checked_add(amount).expect("Tranche amount overflow");
            }
        } else {
            let reserved_slots = if reserved_bytes.is_none() {
                user.inflight_storage_operations as usize
            } else {
                0
            };
            require!(
                request.tranches.len().saturating_add(reserved_slots) < MAX_TRANCHES_PER_REQUEST,
                "Withdrawal tranche limit"
            );
            request.tranches.push(TrancheLock::Unlocked(WithdrawalTranche {
                amount,
                available_epoch_height,
                wnear_residual: 0,
            }));
        }
        let new_total_pending = self
            .data()
            .total_pending_withdrawal_amount
            .checked_add(amount)
            .expect("Pending withdrawal overflow");
        let new_unstake_requested = self
            .data()
            .epoch_requested_unstake_amount
            .checked_add(amount)
            .expect("Requested unstake overflow");
        self.burn_lst(&contract_id, shares, Some("unstake"));
        self.data_mut().total_staked_near_amount -= amount;
        self.data_mut().epoch_requested_unstake_amount = new_unstake_requested;
        self.data_mut().total_pending_withdrawal_amount = new_total_pending;
        self.save_request(&key, Some(request));
        if let Some(reserved) = reserved_bytes {
            self.release_platform_storage(platform_id, reserved);
        }
        self.report_platform_overdraft(platform_id);
        Event::WithdrawalRequestCreated {
            platform_id,
            request_key: key_hash.into(),
            shares: U128(shares),
            amount: U128(amount),
            receiver_id: &message.receiver_id,
            available_epoch_height,
        }
        .emit();
        key_hash
    }

    fn validated_stake_gas(message: &StakeMessage) -> Gas {
        let gas = message.min_gas.unwrap_or(Gas::from_tgas(35));
        // Cap the receiver at 40 Tgas so the 100 Tgas unwrap callback can also reserve 40 Tgas for the resolver.
        require!(
            gas.as_gas() >= Gas::from_tgas(30).as_gas() && gas.as_gas() <= Gas::from_tgas(40).as_gas(),
            "Invalid receiver gas"
        );
        gas
    }

    fn quote_stake_shares(&self, amount: u128) -> Option<u128> {
        let total_staked = self.data().total_staked_near_amount;
        let total_supply = self.data().token.total_supply;
        if total_staked == 0 || total_supply == 0 {
            return None;
        }
        let shares = U256::from(amount) * U256::from(total_supply) / U256::from(total_staked);
        if shares > U256::from(u128::MAX) {
            None
        } else {
            Some(shares.as_u128())
        }
    }

    fn validate_stake_message(&self, platform_id: &AccountId, amount: u128, message: &StakeMessage) -> u64 {
        require!(
            self.data().emergency_storage_reserve >= MIN_EMERGENCY_STORAGE_RESERVE,
            "Emergency storage reserve is below the stake minimum"
        );
        let user = self.require_active_platform(platform_id);
        require!(message.receiver_id == *platform_id, "Stake receiver must be the platform user");
        require!(!message.is_storage_deposit, "Storage deposit is not supported");
        require!(self.data().token.accounts.get(platform_id).is_some(), "Platform FT account is not registered");
        require!(amount >= MIN_STAKE_AMOUNT, "Stake amount is too small");
        require!(self.quote_stake_shares(amount).unwrap_or(0) > 0, "Stake shares round to zero");
        require!(
            self.data().total_staked_near_amount.checked_add(amount).is_some() && self.data().epoch_requested_stake_amount.checked_add(amount).is_some(),
            "Stake amount overflow"
        );
        Self::validated_stake_gas(message);
        require!(message.msg.len() <= MAX_MESSAGE_BYTES, "Message is too long");
        if let Some(memo) = &message.memo {
            require!(memo.len() <= MAX_MESSAGE_BYTES, "Memo is too long");
        }
        let refund = &message.refund_message;
        Self::validate_unstake_message(refund);
        let reservation = Self::checked_reservation(refund);
        require!(
            user.active_request_keys.saturating_add(user.inflight_storage_operations) < MAX_REQUEST_KEYS_PER_PLATFORM
                && self
                    .data()
                    .total_active_request_keys
                    .saturating_add(self.data().total_inflight_storage_operations)
                    < MAX_REQUEST_KEYS_GLOBAL,
            "Withdrawal request limit"
        );
        let key = PlatformRequestKey {
            platform_id: platform_id.clone(),
            request_key: Self::request_key(refund),
        };
        if let Some(existing) = self.data().withdrawal_requests.get(&key) {
            require!(
                existing.tranches.len().saturating_add(user.inflight_storage_operations as usize) < MAX_TRANCHES_PER_REQUEST,
                "Withdrawal tranche limit"
            );
        }
        // Unwrap (10) + unwrap callback (100) + 5 Tgas for admission and scheduling.
        // This is a prepaid-gas admission estimate, not a measured worst-case bound.
        require!(
            env::prepaid_gas().as_gas() >= Gas::from_tgas(115).as_gas(),
            "Insufficient gas for stake Promise chain"
        );
        reservation
    }

    pub(crate) fn load_request(&self, key: &PlatformRequestKey) -> WithdrawalRequest {
        self.platform_user(&key.platform_id);
        let request = self.data().withdrawal_requests.get(key).cloned().expect("Withdrawal request does not exist");
        require!(Self::request_key(&request.message) == key.request_key, "Stored withdrawal route is corrupted");
        require!(!request.tranches.is_empty(), "Withdrawal request has no tranches");
        request
    }

    fn locked_index(request: &WithdrawalRequest) -> usize {
        let mut indices = request.tranches.iter().enumerate().filter(|(_, lock)| lock.is_locked());
        let index = indices.next().expect("Withdrawal request is not locked").0;
        require!(indices.next().is_none(), "Multiple withdrawal locks");
        index
    }

    fn send_wnear(&self, key: PlatformRequestKey, request: &WithdrawalRequest) -> Promise {
        let index = Self::locked_index(request);
        let amount_to_send = match &request.tranches[index] {
            TrancheLock::WnearSending { amount_to_send, .. } => *amount_to_send,
            _ => env::panic_str("Withdrawal is not in wNEAR sending stage"),
        };
        let (msg, memo, receiver_gas) = match &request.message.withdraw_tokens {
            WithdrawTokens::Wnear { msg, memo, min_gas, .. } => (msg.clone(), memo.clone(), min_gas.unwrap_or(Gas::from_tgas(35))),
        };
        let wnear_id = self.configured_wnear_id();
        // The extra 30 Tgas is budgeted for wNEAR's transfer and resolver around the receiver's requested gas.
        let transfer = ext_wnear::ext(wnear_id)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(receiver_gas.checked_add(Gas::from_tgas(30)).unwrap())
            .with_unused_gas_weight(0)
            .ft_transfer_call(request.message.receiver_id.clone(), amount_to_send.into(), memo, msg);
        transfer.then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_ON_WNEAR_TRANSFER)
                .with_unused_gas_weight(0)
                .on_wnear_transfer(key.platform_id, key.request_key.into()),
        )
    }

    fn settle_withdrawal(&mut self, key: &PlatformRequestKey, consumed: u128) {
        let mut request = self.load_request(key);
        let index = Self::locked_index(&request);
        let (mut tranche, sent) = match &request.tranches[index] {
            TrancheLock::WnearSending { tranche, amount_to_send, .. } => (tranche.clone(), *amount_to_send),
            _ => env::panic_str("Withdrawal has no send result"),
        };
        require!(consumed <= sent && consumed <= tranche.amount, "Invalid delivered amount");
        let new_total = self
            .data()
            .total_pending_withdrawal_amount
            .checked_sub(consumed)
            .expect("Pending withdrawal underflow");
        tranche.amount -= consumed;
        let remaining_amount = tranche.amount;
        tranche.wnear_residual = sent - consumed;
        if tranche.amount == 0 {
            request.tranches.remove(index);
        } else {
            request.tranches[index] = TrancheLock::Unlocked(tranche);
        }
        self.data_mut().total_pending_withdrawal_amount = new_total;
        self.save_request(key, if request.tranches.is_empty() { None } else { Some(request) });
        self.report_platform_overdraft(&key.platform_id);
        if consumed > 0 {
            Event::WithdrawalDelivered {
                platform_id: &key.platform_id,
                request_key: key.request_key.into(),
                amount: U128(consumed),
                remaining_amount: U128(remaining_amount),
            }
            .emit();
        }
        if remaining_amount > 0 {
            Event::WithdrawalPartiallyRefunded {
                platform_id: &key.platform_id,
                request_key: key.request_key.into(),
                remaining_amount: U128(remaining_amount),
            }
            .emit();
        }
    }
}

#[near]
impl FungibleTokenReceiver for Contract {
    #[pause]
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        require!(msg.len() <= MAX_MESSAGE_BYTES * 3, "Transfer message is too long");
        let predecessor = env::predecessor_account_id();
        if self.data().wnear_id.as_ref() == Some(&predecessor) {
            let message: StakeMessage = serde_json::from_str(&msg).expect("Invalid stake message");
            let reserved_bytes = self.validate_stake_message(&sender_id, amount.0, &message);
            self.reserve_platform_storage(&sender_id, reserved_bytes);
            let attempt_id = self.data().next_stake_attempt_id;
            self.data_mut().next_stake_attempt_id = attempt_id.checked_add(1).expect("Stake attempt ID overflow");
            let entry = StakeLostFound {
                platform_id: sender_id.clone(),
                amount: amount.0,
                refund_message: message.refund_message.clone(),
                reserved_bytes,
                created_block_height: env::block_height(),
                status: StakeLostFoundStatus::Processing,
            };
            self.save_stake_lostfound(attempt_id, Some(entry), &sender_id);
            let user = self.platform_user(&sender_id);
            require!(
                user.required_storage_balance() <= user.storage_deposit_total,
                "Insufficient platform storage credit"
            );
            self.data_mut().stake_lostfound_near_reserved = self
                .data()
                .stake_lostfound_near_reserved
                .checked_add(amount.0)
                .expect("Lost-found NEAR reserve overflow");
            Event::StakeAccepted {
                attempt_id,
                platform_id: &sender_id,
                receiver_id: &message.receiver_id,
                deposit_amount: amount,
                stake_amount: amount,
            }
            .emit();
            #[cfg(feature = "test")]
            let inject_callback_oog = message.memo.as_deref() == Some("__callback_oog__");
            #[cfg(feature = "test")]
            let callback_gas = if inject_callback_oog { Gas::from_gas(1) } else { GAS_ON_WNEAR_UNWRAPPED };
            #[cfg(not(feature = "test"))]
            let callback_gas = GAS_ON_WNEAR_UNWRAPPED;
            #[cfg(feature = "test")]
            if inject_callback_oog {
                Event::InjectedCallbackGas { gas: callback_gas.as_gas() }.emit();
            }
            let callback = Self::ext(env::current_account_id()).with_static_gas(callback_gas);
            #[cfg(feature = "test")]
            // Keep the injected 1-gas callback from receiving the caller's unused gas.
            let callback = if inject_callback_oog { callback.with_unused_gas_weight(0) } else { callback };
            ext_wnear::ext(self.configured_wnear_id())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(GAS_WNEAR_WITHDRAW)
                .with_unused_gas_weight(0)
                .near_withdraw(amount)
                .then(callback.on_wnear_unwrapped(attempt_id, message));
            // Once accepted, the outer wNEAR resolver must not refund from this contract's shared balance.
            PromiseOrValue::Value(U128(0))
        } else if predecessor == env::current_account_id() {
            let message: UnstakeMessage = serde_json::from_str(&msg).expect("Invalid unstake message");
            self.internal_unstake_shares_to_request(&sender_id, amount.0, &message, None);
            PromiseOrValue::Value(U128(0))
        } else {
            env::panic_str("Invalid token account ID")
        }
    }
}

#[near]
impl Contract {
    #[payable]
    pub fn recover_stake_lostfound(&mut self, attempt_id: u64, asset: StakeRecoveryAsset, evidence: Base58CryptoHash) -> Promise {
        self.assert_owner();
        assert_one_yocto();
        let mut entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(
            match entry.status {
                StakeLostFoundStatus::Processing => true,
                StakeLostFoundStatus::Near => asset == StakeRecoveryAsset::Near,
                StakeLostFoundStatus::Wnear => asset == StakeRecoveryAsset::Wnear,
                StakeLostFoundStatus::Wrapping | StakeLostFoundStatus::Sending => false,
            },
            "Recovery stage is locked or asset contradicts the observed result"
        );
        // For Processing, the owner must verify that every receipt has finished and the asset is still held here.
        require!(
            env::prepaid_gas().as_gas() >= Gas::from_tgas(if asset == StakeRecoveryAsset::Near { 170 } else { 130 }).as_gas(),
            "Insufficient gas for stake recovery"
        );
        let promise = match asset {
            StakeRecoveryAsset::Near => {
                entry.status = StakeLostFoundStatus::Wrapping;
                self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
                ext_wnear::ext(self.configured_wnear_id())
                    .with_attached_deposit(NearToken::from_yoctonear(entry.amount))
                    .with_static_gas(GAS_WNEAR_WRAP)
                    .with_unused_gas_weight(0)
                    .near_deposit()
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_ON_STAKE_RECOVERY_WRAP)
                            .with_unused_gas_weight(0)
                            .on_stake_recovery_wrapped(attempt_id),
                    )
            }
            StakeRecoveryAsset::Wnear => {
                if entry.status == StakeLostFoundStatus::Processing {
                    self.data_mut().stake_lostfound_near_reserved = self
                        .data()
                        .stake_lostfound_near_reserved
                        .checked_sub(entry.amount)
                        .expect("Lost-found NEAR reserve underflow");
                }
                entry.status = StakeLostFoundStatus::Sending;
                self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
                self.send_stake_recovery_wnear(attempt_id, &entry)
            }
        };
        Event::StakeRecoveryAttempt {
            attempt_id,
            platform_id: &entry.platform_id,
            amount: U128(entry.amount),
            asset,
            evidence,
        }
        .emit();
        promise
    }

    #[payable]
    pub fn reconcile_stake_wrapping(&mut self, attempt_id: u64, wrapped: bool, evidence: Base58CryptoHash) {
        self.assert_owner();
        assert_one_yocto();
        let mut entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Wrapping, "Stake recovery is not wrapping");
        entry.status = if wrapped { StakeLostFoundStatus::Wnear } else { StakeLostFoundStatus::Near };
        if wrapped {
            self.data_mut().stake_lostfound_near_reserved = self
                .data()
                .stake_lostfound_near_reserved
                .checked_sub(entry.amount)
                .expect("Lost-found NEAR reserve underflow");
        }
        self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
        Event::StakeRecoveryReconciled {
            attempt_id,
            platform_id: &entry.platform_id,
            asset: Some(if wrapped { StakeRecoveryAsset::Wnear } else { StakeRecoveryAsset::Near }),
            consumed: None,
            evidence,
        }
        .emit();
    }

    #[payable]
    pub fn reconcile_stake_transfer(&mut self, attempt_id: u64, consumed: U128, evidence: Base58CryptoHash) {
        self.assert_owner();
        assert_one_yocto();
        let entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Sending, "Stake recovery is not sending");
        self.settle_stake_recovery_transfer(attempt_id, consumed.0);
        Event::StakeRecoveryReconciled {
            attempt_id,
            platform_id: &entry.platform_id,
            asset: None,
            consumed: Some(consumed),
            evidence,
        }
        .emit();
    }

    pub fn withdraw_request(&mut self, platform_id: AccountId, request_key: Base58CryptoHash) -> Promise {
        let wnear_id = self.configured_wnear_id();
        let key = PlatformRequestKey {
            platform_id,
            request_key: request_key.into(),
        };
        let mut request = self.load_request(&key);
        Self::validate_unstake_message(&request.message);
        let receiver_id = request.message.receiver_id.clone();
        require!(
            request.tranches.iter().all(|lock| !lock.is_locked()),
            "Withdrawal request is already in progress"
        );
        let epoch = get_epoch_height();
        let mut amount = 0u128;
        let mut residual = 0u128;
        let mut latest_epoch = 0u64;
        request.tranches.retain(|lock| {
            if let Some(tranche) = lock.unlocked() {
                if tranche.available_epoch_height <= epoch {
                    amount = amount.checked_add(tranche.amount).expect("Withdrawal amount overflow");
                    residual = residual.checked_add(tranche.wnear_residual).expect("Residual overflow");
                    latest_epoch = latest_epoch.max(tranche.available_epoch_height);
                    return false;
                }
            }
            true
        });
        require!(amount > 0, "Withdrawal request has not matured");
        require!(residual <= amount, "Withdrawal residual exceeds claim");
        let WithdrawTokens::Wnear { min_gas, .. } = &request.message.withdraw_tokens;
        // Wrap path: 10 Tgas near_deposit + 135 Tgas on_near_deposit + 15 Tgas for this call.
        // Residual path: (receiver gas + 30) for ft_transfer_call + 25 for its callback + 10 for this call.
        // These admission margins cover scheduling overhead; they are not measured worst-case bounds.
        let required_gas = if amount > residual {
            Gas::from_tgas(160)
        } else {
            min_gas.unwrap_or(Gas::from_tgas(35)).checked_add(Gas::from_tgas(65)).expect("Gas overflow")
        };
        require!(
            env::prepaid_gas().as_gas() >= required_gas.as_gas(),
            "Insufficient gas for withdrawal Promise chain"
        );
        let tranche = WithdrawalTranche {
            amount,
            available_epoch_height: latest_epoch,
            wnear_residual: residual,
        };
        let amount_to_wrap = amount - residual;
        require!(self.available_near() >= amount_to_wrap, ERR_NO_ENOUGH_CONTRACT_BALANCE);
        let promise = if amount_to_wrap > 0 {
            request.tranches.push(TrancheLock::Wrapping { tranche, amount_to_wrap });
            self.save_request(&key, Some(request));
            require!(self.available_near() >= amount_to_wrap, ERR_NO_ENOUGH_CONTRACT_BALANCE);
            ext_wnear::ext(wnear_id)
                .with_attached_deposit(NearToken::from_yoctonear(amount_to_wrap))
                .with_static_gas(GAS_WNEAR_WRAP)
                .with_unused_gas_weight(0)
                .near_deposit()
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_ON_NEAR_DEPOSIT)
                        .with_unused_gas_weight(0)
                        .on_near_deposit(key.platform_id.clone(), key.request_key.into()),
                )
        } else {
            request.tranches.push(TrancheLock::WnearSending {
                tranche,
                amount_to_send: amount,
            });
            self.save_request(&key, Some(request.clone()));
            self.send_wnear(key.clone(), &request)
        };
        Event::WithdrawalAttempt {
            platform_id: &key.platform_id,
            request_key: key.request_key.into(),
            amount: U128(amount),
            receiver_id: &receiver_id,
        }
        .emit();
        promise
    }

    #[payable]
    pub fn recover_wrapping(&mut self, platform_id: AccountId, request_key: Base58CryptoHash, wrapped: bool, evidence: Base58CryptoHash) {
        self.assert_owner();
        assert_one_yocto();
        let key = PlatformRequestKey {
            platform_id,
            request_key: request_key.into(),
        };
        let mut request = self.load_request(&key);
        let index = Self::locked_index(&request);
        let (mut tranche, amount_to_wrap) = match &request.tranches[index] {
            TrancheLock::Wrapping { tranche, amount_to_wrap } => (tranche.clone(), *amount_to_wrap),
            _ => env::panic_str("Wrapping recovery stage mismatch"),
        };
        if wrapped {
            tranche.wnear_residual = tranche.wnear_residual.checked_add(amount_to_wrap).expect("Wrapped residual overflow");
            require!(tranche.wnear_residual <= tranche.amount, "Wrapped residual exceeds claim");
        }
        request.tranches[index] = TrancheLock::Unlocked(tranche);
        self.save_request(&key, Some(request));
        Event::WithdrawalRecovered {
            platform_id: &key.platform_id,
            request_key,
            evidence,
            wrapped: Some(wrapped),
            consumed: None,
        }
        .emit();
    }

    #[payable]
    pub fn recover_wnear_transfer(&mut self, platform_id: AccountId, request_key: Base58CryptoHash, consumed: U128, evidence: Base58CryptoHash) {
        self.assert_owner();
        assert_one_yocto();
        let key = PlatformRequestKey {
            platform_id,
            request_key: request_key.into(),
        };
        let request = self.load_request(&key);
        let index = Self::locked_index(&request);
        let amount_to_send = match &request.tranches[index] {
            TrancheLock::WnearSending { amount_to_send, .. } => *amount_to_send,
            _ => env::panic_str("wNEAR recovery stage mismatch"),
        };
        require!(consumed.0 <= amount_to_send, "Recovered amount exceeds locked send");
        self.settle_withdrawal(&key, consumed.0);
        Event::WithdrawalRecovered {
            platform_id: &key.platform_id,
            request_key,
            evidence,
            wrapped: None,
            consumed: Some(consumed),
        }
        .emit();
    }

    #[private]
    pub fn on_near_deposit(&mut self, platform_id: AccountId, request_key: Base58CryptoHash) -> PromiseOrValue<()> {
        let key = PlatformRequestKey {
            platform_id,
            request_key: request_key.into(),
        };
        let mut request = self.load_request(&key);
        let index = Self::locked_index(&request);
        let (mut tranche, amount_to_wrap) = match &request.tranches[index] {
            TrancheLock::Wrapping { tranche, amount_to_wrap } => (tranche.clone(), *amount_to_wrap),
            _ => env::panic_str("wNEAR wrapping stage mismatch"),
        };
        if matches!(env::promise_result(0), PromiseResult::Failed) {
            request.tranches[index] = TrancheLock::Unlocked(tranche);
            self.save_request(&key, Some(request));
            Event::WithdrawalFailed {
                platform_id: &key.platform_id,
                request_key,
                stage: "wrapping",
                result: None,
            }
            .emit();
            return PromiseOrValue::Value(());
        }
        tranche.wnear_residual = tranche.wnear_residual.checked_add(amount_to_wrap).expect("Wrapped residual overflow");
        require!(tranche.wnear_residual == tranche.amount, "Wrapped amount differs from claim");
        let amount_to_send = tranche.amount;
        request.tranches[index] = TrancheLock::WnearSending { tranche, amount_to_send };
        self.save_request(&key, Some(request.clone()));
        PromiseOrValue::Promise(self.send_wnear(key, &request))
    }

    #[private]
    pub fn on_wnear_transfer(&mut self, platform_id: AccountId, request_key: Base58CryptoHash) {
        let key = PlatformRequestKey {
            platform_id,
            request_key: request_key.into(),
        };
        let request = self.load_request(&key);
        let index = Self::locked_index(&request);
        let amount_to_send = match &request.tranches[index] {
            TrancheLock::WnearSending { amount_to_send, .. } => *amount_to_send,
            _ => env::panic_str("wNEAR send stage mismatch"),
        };
        match env::promise_result(0) {
            PromiseResult::Successful(bytes) => {
                if let Ok(consumed) = serde_json::from_slice::<U128>(&bytes) {
                    if consumed.0 <= amount_to_send {
                        self.settle_withdrawal(&key, consumed.0);
                        return;
                    }
                }
            }
            PromiseResult::Failed => {}
        }
        Event::WithdrawalFailed {
            platform_id: &key.platform_id,
            request_key,
            stage: "wnear_sending",
            result: Some("unclear"),
        }
        .emit();
    }

    #[private]
    pub fn on_stake_recovery_wrapped(&mut self, attempt_id: u64) -> PromiseOrValue<()> {
        let mut entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Wrapping, "Stake recovery is not wrapping");
        if matches!(env::promise_result(0), PromiseResult::Failed) {
            entry.status = StakeLostFoundStatus::Near;
            self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
            Event::StakeRecoveryFailed {
                attempt_id,
                platform_id: &entry.platform_id,
                stage: "wrapping",
            }
            .emit();
            return PromiseOrValue::Value(());
        }
        self.data_mut().stake_lostfound_near_reserved = self
            .data()
            .stake_lostfound_near_reserved
            .checked_sub(entry.amount)
            .expect("Lost-found NEAR reserve underflow");
        entry.status = StakeLostFoundStatus::Sending;
        self.save_stake_lostfound(attempt_id, Some(entry.clone()), &entry.platform_id);
        PromiseOrValue::Promise(self.send_stake_recovery_wnear(attempt_id, &entry))
    }

    #[private]
    pub fn on_stake_recovery_transfer(&mut self, attempt_id: u64) {
        let entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Sending, "Stake recovery is not sending");
        if let PromiseResult::Successful(bytes) = env::promise_result(0) {
            if let Ok(consumed) = serde_json::from_slice::<U128>(&bytes) {
                if consumed.0 <= entry.amount {
                    self.settle_stake_recovery_transfer(attempt_id, consumed.0);
                    return;
                }
            }
        }
        Event::StakeRecoveryFailed {
            attempt_id,
            platform_id: &entry.platform_id,
            stage: "sending_unclear",
        }
        .emit();
    }

    #[private]
    pub fn on_wnear_unwrapped(&mut self, attempt_id: u64, message: StakeMessage) {
        let mut entry = self.data().stake_lostfound.get(&attempt_id).cloned().expect("Stake attempt does not exist");
        require!(entry.status == StakeLostFoundStatus::Processing, "Stake attempt is not processing");
        require!(entry.refund_message == message.refund_message, "Stake refund route mismatch");
        let platform_id = entry.platform_id.clone();
        let amount = U128(entry.amount);
        let reserved_bytes = entry.reserved_bytes;
        if matches!(env::promise_result(0), PromiseResult::Failed) {
            entry.status = StakeLostFoundStatus::Wnear;
            self.save_stake_lostfound(attempt_id, Some(entry), &platform_id);
            self.data_mut().stake_lostfound_near_reserved = self
                .data()
                .stake_lostfound_near_reserved
                .checked_sub(amount.0)
                .expect("Lost-found NEAR reserve underflow");
            Event::StakeLostFoundDetected {
                attempt_id,
                platform_id: &platform_id,
                amount,
                status: StakeLostFoundStatus::Wnear,
            }
            .emit();
            return;
        }
        #[cfg(feature = "test")]
        if message.memo.as_deref() == Some("__panic_after_unwrap__") {
            env::panic_str("Injected post-unwrap callback failure");
        }
        #[cfg(feature = "test")]
        let shares = if message.memo.as_deref() == Some("__force_rewrap__") {
            0
        } else {
            self.quote_stake_shares(amount.0).unwrap_or(0)
        };
        #[cfg(not(feature = "test"))]
        let shares = self.quote_stake_shares(amount.0).unwrap_or(0);
        if shares == 0
            || self.data().total_staked_near_amount.checked_add(amount.0).is_none()
            || self.data().epoch_requested_stake_amount.checked_add(amount.0).is_none()
        {
            entry.status = StakeLostFoundStatus::Near;
            self.save_stake_lostfound(attempt_id, Some(entry), &platform_id);
            Event::StakeLostFoundDetected {
                attempt_id,
                platform_id: &platform_id,
                amount,
                status: StakeLostFoundStatus::Near,
            }
            .emit();
            return;
        }
        let contract_id = env::current_account_id();
        self.mint_lst(&contract_id, shares, Some("stake"));
        self.data_mut().total_staked_near_amount += amount.0;
        self.data_mut().epoch_requested_stake_amount += amount.0;
        self.data_mut()
            .token
            .internal_transfer(&contract_id, &message.receiver_id, shares, message.memo.clone());
        self.remove_stake_lostfound(attempt_id, &entry);
        Event::StakeCompleted {
            attempt_id,
            platform_id: &platform_id,
            receiver_id: &message.receiver_id,
            stake_amount: amount,
            shares: U128(shares),
        }
        .emit();
        let receiver_gas = Self::validated_stake_gas(&message);
        ext_platform_receiver::ext(message.receiver_id.clone())
            .with_static_gas(receiver_gas)
            .with_unused_gas_weight(0)
            .ft_on_transfer(platform_id.clone(), shares.into(), message.msg)
            .then(
                Self::ext(contract_id)
                    .with_static_gas(GAS_STAKE_RESOLVER)
                    .with_unused_gas_weight(0)
                    .on_stake_delivery_resolved(platform_id, message.receiver_id, shares.into(), message.refund_message, reserved_bytes),
            );
    }

    #[private]
    pub fn on_stake_delivery_resolved(
        &mut self,
        platform_id: AccountId,
        receiver_id: AccountId,
        shares: U128,
        refund_message: UnstakeMessage,
        reserved_bytes: u64,
    ) {
        let contract_id = env::current_account_id();
        let (used, burned) = self.data_mut().token.internal_ft_resolve_transfer(&contract_id, receiver_id.clone(), shares);
        require!(burned == 0, "Contract FT account was unregistered");
        let refunded = shares.0.checked_sub(used).expect("Invalid FT resolver result");
        if refunded > 0 {
            self.internal_unstake_shares_to_request(&platform_id, refunded, &refund_message, Some(reserved_bytes));
            Event::PlatformStakeDeliveryRefunded {
                platform_id: &platform_id,
                receiver_id: &receiver_id,
                refunded_shares: U128(refunded),
            }
            .emit();
        } else {
            self.release_platform_storage(&platform_id, reserved_bytes);
            Event::PlatformStakeDelivered {
                platform_id: &platform_id,
                receiver_id: &receiver_id,
                shares: U128(shares.0),
            }
            .emit();
        }
    }
}

#[near]
impl Contract {
    pub fn get_stake_lostfound(&self, attempt_id: u64) -> Option<StakeLostFoundView> {
        self.data().stake_lostfound.get(&attempt_id).map(|entry| StakeLostFoundView {
            attempt_id,
            platform_id: entry.platform_id.clone(),
            amount: U128(entry.amount),
            refund_message: entry.refund_message.clone(),
            created_block_height: entry.created_block_height,
            status: entry.status,
        })
    }

    pub fn get_stake_lostfound_entries(&self, from_index: Option<usize>, limit: Option<usize>) -> Vec<StakeLostFoundView> {
        self.data()
            .stake_lostfound
            .keys()
            .skip(from_index.unwrap_or(0))
            .take(limit.unwrap_or(50).min(100))
            .filter_map(|attempt_id| self.get_stake_lostfound(*attempt_id))
            .collect()
    }

    pub fn get_stake_lostfound_near_reserved(&self) -> U128 {
        U128(self.data().stake_lostfound_near_reserved)
    }

    pub fn get_emergency_storage_reserve(&self) -> U128 {
        U128(self.data().emergency_storage_reserve)
    }

    #[payable]
    pub fn deposit_emergency_storage_reserve(&mut self) {
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Emergency storage reserve deposit must be positive");
        let total = self
            .data()
            .emergency_storage_reserve
            .checked_add(amount)
            .expect("Emergency storage reserve overflow");
        self.data_mut().emergency_storage_reserve = total;
        Event::EmergencyStorageReserveDeposited {
            account_id: &env::predecessor_account_id(),
            amount: U128(amount),
            total: U128(total),
        }
        .emit();
    }

    pub fn get_wnear_id(&self) -> Option<AccountId> {
        self.data().wnear_id.clone()
    }

    #[payable]
    pub fn set_wnear_id(&mut self, wnear_id: AccountId) {
        self.assert_owner();
        assert_one_yocto();
        require!(self.data().wnear_id.is_none(), "wNEAR is already configured");
        self.data_mut().wnear_id = Some(wnear_id.clone());
        Event::WnearConfigured { wnear_id: &wnear_id }.emit();
    }

    #[payable]
    pub fn add_platform_user(&mut self, platform_id: AccountId, refund_account_id: AccountId) {
        self.assert_owner();
        assert_one_yocto();
        require!(self.data().platform_users.get(&platform_id).is_none(), "Platform user already exists");
        require!(
            self.available_near() >= self.storage_balance_bounds().min.as_yoctonear(),
            "Insufficient management reserve"
        );
        let sponsored_ft_registration = self.data().token.accounts.get(&platform_id).is_none();
        if sponsored_ft_registration {
            if self.data().accounts.get(&platform_id).is_none() {
                self.data_mut().accounts.insert(platform_id.clone(), Account::default());
            }
            self.data_mut().token.internal_register_account(&platform_id);
        } else if self.data().accounts.get(&platform_id).is_none() {
            self.data_mut().accounts.insert(platform_id.clone(), Account::default());
        }
        self.save_platform_user(
            &platform_id,
            PlatformUser {
                status: PlatformUserStatus::Active,
                refund_account_id,
                sponsored_ft_registration,
                storage_deposit_total: 0,
                storage_used_bytes: 0,
                storage_reserved_bytes: 0,
                storage_refund_pending: 0,
                inflight_storage_operations: 0,
                active_request_keys: 0,
            },
        );
        Event::PlatformUserAdded { platform_id: &platform_id }.emit();
    }

    #[payable]
    pub fn set_platform_user_draining(&mut self, platform_id: AccountId) {
        self.assert_owner();
        assert_one_yocto();
        let mut user = self.platform_user(&platform_id);
        user.status = PlatformUserStatus::Draining;
        self.save_platform_user(&platform_id, user);
        Event::PlatformUserDraining { platform_id: &platform_id }.emit();
    }

    #[payable]
    pub fn remove_platform_user(&mut self, platform_id: AccountId) {
        self.assert_owner();
        assert_one_yocto();
        let user = self.platform_user(&platform_id);
        require!(user.status == PlatformUserStatus::Draining, "Platform user is active");
        require!(
            user.active_request_keys == 0
                && user.inflight_storage_operations == 0
                && user.storage_used_bytes == 0
                && user.storage_reserved_bytes == 0
                && user.storage_refund_pending == 0
                && user.storage_deposit_total == 0,
            "Platform user is not empty"
        );
        if user.sponsored_ft_registration {
            require!(self.data().token.accounts.get(&platform_id) == Some(0), "Sponsored FT account has a balance");
            require!(
                self.internal_get_account(&platform_id).unstaked == 0,
                "Sponsored account has an unstaked balance"
            );
            self.data_mut().token.accounts.remove(&platform_id);
            self.data_mut().accounts.remove(&platform_id);
        }
        self.data_mut().platform_users.remove(&platform_id);
        Event::PlatformUserRemoved { platform_id: &platform_id }.emit();
    }

    #[payable]
    pub fn deposit_platform_user_storage(&mut self, platform_id: AccountId) {
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Storage deposit must be positive");
        let mut user = self.platform_user(&platform_id);
        user.storage_deposit_total = user.storage_deposit_total.checked_add(amount).expect("Storage credit overflow");
        self.data_mut().total_platform_user_storage_deposits = self
            .data()
            .total_platform_user_storage_deposits
            .checked_add(amount)
            .expect("Global storage credit overflow");
        self.save_platform_user(&platform_id, user);
        Event::PlatformUserStorageDeposited {
            platform_id: &platform_id,
            amount: U128(amount),
        }
        .emit();
    }

    pub fn withdraw_unused_platform_user_storage(&mut self, platform_id: AccountId, amount: U128) -> Promise {
        let mut user = self.platform_user(&platform_id);
        require!(
            env::predecessor_account_id() == user.refund_account_id,
            "Only refund account can withdraw credit"
        );
        require!(amount.0 > 0, "Withdrawal amount must be positive");
        let free = user
            .storage_deposit_total
            .checked_sub(user.required_storage_balance())
            .and_then(|n| n.checked_sub(user.storage_refund_pending))
            .unwrap_or(0);
        require!(free >= amount.0, "Insufficient unused storage credit");
        user.storage_refund_pending = user.storage_refund_pending.checked_add(amount.0).unwrap();
        let receiver = user.refund_account_id.clone();
        self.save_platform_user(&platform_id, user);
        Promise::new(receiver).transfer(NearToken::from_yoctonear(amount.0)).then(
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::from_tgas(10))
                .with_unused_gas_weight(0)
                .on_platform_storage_refund(platform_id, amount),
        )
    }

    #[private]
    pub fn on_platform_storage_refund(&mut self, platform_id: AccountId, amount: U128) {
        let mut user = self.platform_user(&platform_id);
        user.storage_refund_pending = user.storage_refund_pending.checked_sub(amount.0).expect("Invalid pending refund");
        if matches!(env::promise_result(0), PromiseResult::Successful(_)) {
            user.storage_deposit_total = user.storage_deposit_total.checked_sub(amount.0).expect("Invalid credit refund");
            self.data_mut().total_platform_user_storage_deposits = self
                .data()
                .total_platform_user_storage_deposits
                .checked_sub(amount.0)
                .expect("Invalid global credit refund");
            Event::PlatformUserStorageWithdrawn {
                platform_id: &platform_id,
                amount,
            }
            .emit();
        }
        self.save_platform_user(&platform_id, user);
    }

    pub fn get_platform_user(&self, platform_id: AccountId) -> Option<PlatformUserView> {
        self.data().platform_users.get(&platform_id).map(|user| PlatformUserView {
            platform_id,
            status: user.status,
            refund_account_id: user.refund_account_id.clone(),
            sponsored_ft_registration: user.sponsored_ft_registration,
            storage_deposit_total: user.storage_deposit_total.into(),
            storage_used_bytes: user.storage_used_bytes,
            storage_reserved_bytes: user.storage_reserved_bytes,
            storage_refund_pending: user.storage_refund_pending.into(),
            inflight_storage_operations: user.inflight_storage_operations,
            active_request_keys: user.active_request_keys,
            required_storage_balance: user.required_storage_balance().into(),
            storage_overdraft: user.overdraft().into(),
        })
    }

    pub fn get_platform_users(&self, from_index: Option<usize>, limit: Option<usize>) -> Vec<PlatformUserView> {
        self.data()
            .platform_users
            .keys()
            .skip(from_index.unwrap_or(0))
            .take(limit.unwrap_or(50).min(100))
            .filter_map(|id| self.get_platform_user(id.clone()))
            .collect()
    }

    pub fn get_withdrawal_request(&self, platform_id: AccountId, request_key: Base58CryptoHash) -> Option<WithdrawalRequestView> {
        let key = PlatformRequestKey {
            platform_id: platform_id.clone(),
            request_key: request_key.into(),
        };
        self.data().withdrawal_requests.get(&key).map(|request| {
            require!(
                Self::request_key(&request.message) == key.request_key && !request.tranches.is_empty(),
                "Stored withdrawal request is corrupted"
            );
            WithdrawalRequestView {
                platform_id,
                request_key,
                tranches: request
                    .tranches
                    .iter()
                    .map(|lock| {
                        let tranche = match lock {
                            TrancheLock::Unlocked(t) | TrancheLock::Wrapping { tranche: t, .. } | TrancheLock::WnearSending { tranche: t, .. } => t,
                        };
                        WithdrawalTrancheView {
                            amount: tranche.amount.into(),
                            available_epoch_height: tranche.available_epoch_height,
                            wnear_residual: tranche.wnear_residual.into(),
                            matured: tranche.available_epoch_height <= get_epoch_height(),
                            locked: lock.is_locked(),
                        }
                    })
                    .collect(),
            }
        })
    }

    pub fn get_request_key(&self, args: UnstakeMessage) -> Base58CryptoHash {
        Self::request_key(&args).into()
    }

    pub fn get_withdrawal_request_tranches(&self, platform_id: AccountId, request_key: Base58CryptoHash) -> Option<Vec<WithdrawalTrancheView>> {
        self.get_withdrawal_request(platform_id, request_key).map(|request| request.tranches)
    }

    pub fn get_withdrawal_request_tranches_by_args(&self, platform_id: AccountId, args: UnstakeMessage) -> Option<Vec<WithdrawalTrancheView>> {
        self.get_withdrawal_request_tranches(platform_id, Self::request_key(&args).into())
    }

    pub fn get_withdrawal_requests_count(&self, platform_id: AccountId) -> u32 {
        self.data().platform_users.get(&platform_id).map_or(0, |user| user.active_request_keys)
    }

    pub fn get_total_pending_withdrawal_amount(&self) -> U128 {
        self.data().total_pending_withdrawal_amount.into()
    }

    pub fn get_withdrawal_requests(&self, platform_id: AccountId, from_index: Option<usize>, limit: Option<usize>) -> Vec<WithdrawalRequestView> {
        self.data()
            .withdrawal_requests
            .keys()
            .filter(|key| key.platform_id == platform_id)
            .skip(from_index.unwrap_or(0))
            .take(limit.unwrap_or(50).min(100))
            .filter_map(|key| self.get_withdrawal_request(platform_id.clone(), key.request_key.into()))
            .collect()
    }
}

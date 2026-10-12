use crate::*;

impl Contract {
    pub(crate) fn reconcile_reward_balance(
        &mut self,
        mut validator: Validator,
        account: HumanReadableAccount,
    ) {
        if !validator.executing {
            return;
        }
        let old_total = validator.total_balance();
        let new_total = account.staked_balance.0 + account.unstaked_balance.0;

        if new_total < old_total {
            let loss = old_total - new_total;
            let old_staked = validator.staked_amount;
            let old_unstaked = validator.unstaked_amount;
            let share_assets = self.data().total_staked_near_amount;
            let pool = &mut self.data_mut().validator_pool;
            pool.total_weight -= validator.weight;
            pool.total_base_stake_amount -= validator.base_stake_amount;
            validator.staked_amount = account.staked_balance.0;
            validator.unstaked_amount = account.unstaked_balance.0;
            validator.weight = 0;
            validator.base_stake_amount = 0;
            validator.executing = false;
            pool.save_validator(&validator);
            self.data_mut().total_staked_near_amount = share_assets.saturating_sub(loss);
            Event::EpochUpdateLoss {
                validator_id: &validator.account_id,
                old_staked_balance: &old_staked.into(),
                old_unstaked_balance: &old_unstaked.into(),
                new_staked_balance: &account.staked_balance,
                new_unstaked_balance: &account.unstaked_balance,
                loss: &loss.into(),
                uncovered_loss: &loss.saturating_sub(share_assets).into(),
                total_staked_near_amount: &self.data().total_staked_near_amount.into(),
            }
            .emit();
            return;
        }
        let rewards = new_total - old_total;
        let old_base = validator.base_stake_amount;
        let new_base = if old_base == 0 || old_total == 0 {
            old_base
        } else {
            (U256::from(old_base) * U256::from(new_total) / U256::from(old_total)).as_u128()
        };
        let new_pool_base =
            self.data().validator_pool.total_base_stake_amount + new_base - old_base;

        validator.staked_amount = account.staked_balance.0;
        validator.unstaked_amount = account.unstaked_balance.0;
        validator.base_stake_amount = new_base;
        validator.executing = false;
        self.data_mut().validator_pool.total_base_stake_amount = new_pool_base;
        self.data_mut().validator_pool.save_validator(&validator);
        self.data_mut().total_staked_near_amount += rewards;
        Event::EpochUpdateRewards {
            validator_id: &validator.account_id,
            old_balance: &old_total.into(),
            new_balance: &new_total.into(),
            rewards: &rewards.into(),
        }
        .emit();
        if rewards > 0 {
            self.internal_distribute_staking_rewards(rewards);
        }
    }

    pub(crate) fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.data().owner_id,
            ERR_NOT_OWNER
        );
    }

    pub(crate) fn internal_get_beneficiaries(&self) -> HashMap<AccountId, u32> {
        self.data()
            .beneficiaries
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// When there are rewards, a part of them will be
    /// given to executor, manager or treasury by minting new LST tokens.
    pub(crate) fn internal_distribute_staking_rewards(&mut self, rewards: u128) {
        let hashmap = self.internal_get_beneficiaries();
        let total_bps = hashmap.values().sum::<u32>();
        if total_bps > 0 {
            let total_reward_near_amount = bps_mul(rewards, total_bps);
            let total_reward_shares =
                self.num_shares_from_staked_amount_rounded_down(total_reward_near_amount);
            let mut remain_reward_shares = total_reward_shares;
            let mut hashmap_iter = hashmap.iter().peekable();
            while let Some((account_id, bps)) = hashmap_iter.next() {
                if hashmap_iter.peek().is_none() {
                    if remain_reward_shares > 0 {
                        self.mint_lst(
                            account_id,
                            remain_reward_shares,
                            Some("beneficiary rewards"),
                        );
                    }
                } else {
                    let reward_shares = total_reward_shares * *bps as u128 / total_bps as u128;
                    if reward_shares > 0 {
                        self.mint_lst(account_id, reward_shares, Some("beneficiary rewards"));
                        remain_reward_shares -= reward_shares;
                    }
                }
            }
        }
    }
}

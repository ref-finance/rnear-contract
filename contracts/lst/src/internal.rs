use crate::*;

impl Contract {
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

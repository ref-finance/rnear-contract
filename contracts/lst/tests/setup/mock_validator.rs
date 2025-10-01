use crate::*;

pub struct MockValidatorContract(pub Contract);

impl MockValidatorContract {
    pub async fn get_account_staked_balance(&self, account_id: &AccountId) -> Result<U128> {
        self.0
            .call("get_account_staked_balance")
            .args_json(json!({
                "account_id": account_id,
            }))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }

    pub async fn get_account_unstaked_balance(&self, account_id: &AccountId) -> Result<U128> {
        self.0
            .call("get_account_unstaked_balance")
            .args_json(json!({
                "account_id": account_id,
            }))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }
}

/// test stub
impl MockValidatorContract {
    pub async fn set_balance_delta(
        &self,
        caller: &Account,
        staked_delta: u128,
        unstaked_delta: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_balance_delta")
            .args_json(json!({
                "staked_delta": staked_delta.to_string(),
                "unstaked_delta": unstaked_delta.to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn add_reward(&self, caller: &Account, amount: u128) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_reward")
            .args_json(json!({
                "amount": amount.to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn set_get_account_fail(
        &self,
        caller: &Account,
        value: bool,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_get_account_fail")
            .args_json(json!({
                "value": value
            }))
            .max_gas()
            .transact()
            .await
    }
}

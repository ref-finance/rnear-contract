use crate::*;

pub struct LstContract(pub Contract);

/// basic info
impl LstContract {
    pub async fn get_total_staked_balance(&self) -> Result<U128> {
        self.0
            .call("get_total_staked_balance")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }

    pub async fn ft_price(&self) -> Result<U128> {
        self.0
            .call("ft_price")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }

    pub async fn get_account_details(&self, account_id: &AccountId) ->Result<AccountDetailsView> {
        self.0
            .call("get_account_details")
            .args_json(json!({
                "account_id": account_id
            }))
            .view()
            .await
            .unwrap()
            .json::<AccountDetailsView>()
    }
}

/// ft
impl LstContract {
    pub async fn ft_total_supply(&self) -> Result<U128> {
        self.0
            .call("ft_total_supply")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }

    pub async fn ft_metadata(&self) -> Result<FungibleTokenMetadata> {
        self.0
            .call("ft_metadata")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<FungibleTokenMetadata>()
    }

    pub async fn ft_transfer(
        &self,
        caller: &Account,
        receiver_id: &AccountId,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "ft_transfer")
            .args_json(json!({
                "receiver_id": receiver_id,
                "amount": amount.to_string(),
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn ft_transfer_call(
        &self,
        caller: &Account,
        receiver_id: &AccountId,
        amount: u128,
        msg: String,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "ft_transfer_call")
            .args_json(json!({
                "receiver_id": receiver_id,
                "amount": amount.to_string(),
                "msg": msg,
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn ft_balance_of(&self, account_id: &AccountId) -> Result<U128> {
        self.0
            .call("ft_balance_of")
            .args_json(json!({
                "account_id": account_id,
            }))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }
}

/// storage
impl LstContract {
    pub async fn storage_balance_of(&self, account_id: &AccountId) -> Result<StorageBalance> {
        self.0
            .call("storage_balance_of")
            .args_json(json!({
                "account_id": account_id,
            }))
            .view()
            .await
            .unwrap()
            .json::<StorageBalance>()
    }

    pub async fn storage_deposit(
        &self,
        caller: &Account,
        account_id: Option<&AccountId>,
        deposit_amount: u128,
    ) -> Result<ExecutionFinalResult> {
        if account_id.is_some() {
            caller
            .call(self.0.id(), "storage_deposit")
            .args_json(json!({
                "account_id": account_id
            }))
            .deposit(NearToken::from_yoctonear(deposit_amount))
            .max_gas()
            .transact()
            .await
        } else {
            caller
            .call(self.0.id(), "storage_deposit")
            .args_json(json!({}))
            .deposit(NearToken::from_yoctonear(deposit_amount))
            .max_gas()
            .transact()
            .await
        }
    }

    pub async fn storage_unregister(
        &self,
        caller: &Account,
        force: Option<bool>,
    ) -> Result<ExecutionFinalResult> {
        if force.is_some() {
            caller
            .call(self.0.id(), "storage_unregister")
            .args_json(json!({
                "force": force.unwrap()
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
        } else {
            caller
            .call(self.0.id(), "storage_unregister")
            .args_json(json!({}))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
        }
    }
}

/// owner and manager interfaces
impl LstContract {
    pub async fn set_beneficiary(
        &self,
        caller: &Account,
        account_id: &AccountId,
        bps: u32,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_beneficiary")
            .args_json(json!({
                "account_id": account_id,
                "bps": bps
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }
}

/// test stub
impl LstContract {
    pub async fn set_epoch_height(
        &self,
        caller: &Account,
        epoch: u64,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_epoch_height")
            .args_json(json!({
                "epoch": epoch
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn add_epoch_rewards(
        &self,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_epoch_rewards")
            .args_json(json!({
                "amount": amount.to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn read_epoch_height(&self) -> Result<u64> {
        self.0
            .call("read_epoch_height")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<u64>()
    }
}

/// stake pool interfaces
impl LstContract {
    pub async fn deposit(
        &self,
        caller: &Account,
        near_balance: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "deposit")
            .args_json(json!({}))
            .deposit(NearToken::from_near(near_balance))
            .max_gas()
            .transact()
            .await
    }

    pub async fn stake(
        &self,
        caller: &Account,
        near_balance: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "stake")
            .args_json(json!({
                "amount": NearToken::from_near(near_balance).as_yoctonear().to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn stake_all(&self, caller: &Account) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "stake_all")
            .args_json(json!({}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn deposit_and_stake(
        &self,
        caller: &Account,
        near_balance: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "deposit_and_stake")
            .args_json(json!({}))
            .deposit(NearToken::from_near(near_balance))
            .max_gas()
            .transact()
            .await
    }

    pub async fn unstake(
        &self,
        caller: &Account,
        near_balance: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "unstake")
            .args_json(json!({
                "amount": NearToken::from_near(near_balance).as_yoctonear().to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn unstake_in_yocto(
        &self,
        caller: &Account,
        amount: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "unstake")
            .args_json(json!({
                "amount": amount.to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn unstake_all(&self, caller: &Account) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "unstake_all")
            .args_json(json!({}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn withdraw(
        &self,
        caller: &Account,
        near_balance: u128,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "withdraw")
            .args_json(json!({
                "amount": NearToken::from_near(near_balance).as_yoctonear().to_string()
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn withdraw_all(&self, caller: &Account) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "withdraw_all")
            .args_json(json!({}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn get_account_total_balance(&self, account_id: &AccountId) -> Result<U128> {
        self.0
            .call("get_account_total_balance")
            .args_json(json!({
                "account_id": account_id
            }))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }

    pub async fn get_account_staked_balance(&self, account_id: &AccountId) -> Result<U128> {
        self.0
            .call("get_account_staked_balance")
            .args_json(json!({
                "account_id": account_id
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
                "account_id": account_id
            }))
            .view()
            .await
            .unwrap()
            .json::<U128>()
    }
}

/// validator related
impl LstContract {
    pub async fn get_validator(
        &self,
        validator_id: &AccountId,
    ) -> Result<Option<lst::ValidatorInfo>> {
        self.0
            .call("get_validator")
            .args_json(json!({
                "validator_id": validator_id,
            }))
            .view()
            .await
            .unwrap()
            .json::<Option<lst::ValidatorInfo>>()
    }

    pub async fn get_validators(
        &self,
        from_index: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<lst::ValidatorInfo>> {
        let mut args = HashMap::new();
        if from_index.is_some() {
            args.insert("from_index", from_index.unwrap());
        }
        if limit.is_some() {
            args.insert("limit", limit.unwrap());
        }
        self.0
            .call("get_validators")
            .args_json(json!(args))
            .view()
            .await
            .unwrap()
            .json::<Vec<lst::ValidatorInfo>>()
    }

    pub async fn get_total_weight(&self) -> Result<u16> {
        self.0
            .call("get_total_weight")
            .args_json(json!({}))
            .view()
            .await
            .unwrap()
            .json::<u16>()
    }

    pub async fn add_validator(
        &self,
        caller: &Account,
        validator_id: &AccountId,
        weight: u16,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_validator")
            .args_json(json!({
                "validator_id": validator_id,
                "weight": weight
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn add_validators(
        &self,
        caller: &Account,
        validator_ids: Vec<&AccountId>,
        weights: Vec<u16>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "add_validators")
            .args_json(json!({
                "validator_ids": validator_ids,
                "weights": weights
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn remove_validator(
        &self,
        caller: &Account,
        validator_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "remove_validator")
            .args_json(json!({
                "validator_id": validator_id
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_weight(
        &self,
        caller: &Account,
        validator_id: &AccountId,
        weight: u16,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_weight")
            .args_json(json!({
                "validator_id": validator_id,
                "weight": weight
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_weights(
        &self,
        caller: &Account,
        validator_ids: Vec<&AccountId>,
        weights: Vec<u16>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_weights")
            .args_json(json!({
                "validator_ids": validator_ids,
                "weights": weights
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn update_base_stake_amounts(
        &self,
        caller: &Account,
        validator_ids: Vec<&AccountId>,
        amounts: Vec<u128>,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "update_base_stake_amounts")
            .args_json(json!({
                "validator_ids": validator_ids,
                "amounts": amounts.iter().map(|x| x.to_string()).collect::<Vec<_>>()
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn set_whitelist_contract_id(
        &self,
        caller: &Account,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "set_whitelist_contract_id")
            .args_json(json!({
                "account_id": account_id
            }))
            .deposit(NearToken::from_yoctonear(1))
            .max_gas()
            .transact()
            .await
    }

    pub async fn drain_unstake(
        &self,
        caller: &Account,
        validator_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "drain_unstake")
            .args_json(json!({
                "validator_id": validator_id
            }))
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
    }

    pub async fn drain_withdraw(
        &self,
        caller: &Account,
        validator_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "drain_withdraw")
            .args_json(json!({
                "validator_id": validator_id
            }))
            .max_gas()
            .transact()
            .await
    }
}

/// epoch operation related
impl LstContract {
    pub async fn epoch_stake(&self, caller: &Account) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "epoch_stake")
            .args_json(json!({}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn epoch_unstake(&self, caller: &Account) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "epoch_unstake")
            .args_json(json!({}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn epoch_update_rewards(&self, caller: &Account, validator_id: &AccountId) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "epoch_update_rewards")
            .args_json(json!({
                "validator_id": validator_id
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn sync_balance_from_validator(&self, caller: &Account, validator_id: &AccountId) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "sync_balance_from_validator")
            .args_json(json!({ "validator_id": validator_id }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn epoch_withdraw(&self, caller: &Account, validator_id: &AccountId) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "epoch_withdraw")
            .args_json(json!({
                "validator_id": validator_id
            }))
            .max_gas()
            .transact()
            .await
    }
}

/// upgrade related
impl LstContract {
    pub async fn up_stage_code(
        &self,
        account: &Account,
        wasm_path: &str,
    ) -> Result<ExecutionFinalResult> {
        account
            .call(self.0.id(), "up_stage_code")
            .args(std::fs::read(wasm_path).unwrap())
            .max_gas()
            .transact()
            .await
    }

    pub async fn up_deploy_code(
        &self,
        account: &Account,
        staged_code_hash: String,
    ) -> Result<ExecutionFinalResult> {
        account
            .call(self.0.id(), "up_deploy_code")
            .args_json(json!({"hash":  staged_code_hash,
            "function_call_args": Some(near_plugins::upgradable::FunctionCallArgs{
                function_name: "migrate_state".to_string(),
                arguments: vec![],
                amount: NearToken::from_near(0),
                gas: Gas::from_tgas(20)
            })}))
            .max_gas()
            .transact()
            .await
    }

    pub async fn up_staged_code_hash(&self) -> Result<Option<String>> {
        self.0
            .call("up_staged_code_hash")
            .view()
            .await
            .unwrap()
            .json::<Option<String>>()
    }

    pub async fn get_version(&self) -> Result<String> {
        self.0
            .call("get_version")
            .view()
            .await
            .unwrap()
            .json::<String>()
    }
}

/// role related
impl LstContract {
    pub async fn acl_get_super_admins(&self) -> Result<Vec<AccountId>> {
        self.0
            .call("acl_get_super_admins")
            .args_json(json!({
                "skip": 0,
                "limit": u64::MAX.to_string(),
            }))
            .view()
            .await
            .unwrap()
            .json::<Vec<AccountId>>()
    }

    pub async fn acl_get_grantees(&self, role: String) -> Result<Vec<AccountId>> {
        self.0
            .call("acl_get_grantees")
            .args_json(json!({
                "role": role,
                "skip": 0,
                "limit": u64::MAX.to_string(),
            }))
            .view()
            .await
            .unwrap()
            .json::<Vec<AccountId>>()
    }

    pub async fn acl_add_super_admin(
        &self,
        caller: &Account,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "acl_add_super_admin")
            .args_json(json!({
                "account_id": account_id
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn acl_revoke_super_admin(
        &self,
        caller: &Account,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "acl_revoke_super_admin")
            .args_json(json!({
                "account_id": account_id
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn acl_grant_role(
        &self,
        caller: &Account,
        role: String,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "acl_grant_role")
            .args_json(json!({
                "role": role,
                "account_id": account_id
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn acl_revoke_role(
        &self,
        caller: &Account,
        role: String,
        account_id: &AccountId,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "acl_revoke_role")
            .args_json(json!({
                "role": role,
                "account_id": account_id
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn pa_pause_feature(
        &self,
        caller: &Account,
        key: String,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "pa_pause_feature")
            .args_json(json!({
                "key": key,
            }))
            .max_gas()
            .transact()
            .await
    }

    pub async fn pa_unpause_feature(
        &self,
        caller: &Account,
        key: String,
    ) -> Result<ExecutionFinalResult> {
        caller
            .call(self.0.id(), "pa_unpause_feature")
            .args_json(json!({
                "key": key,
            }))
            .max_gas()
            .transact()
            .await
    }
}

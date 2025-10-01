use crate::*;

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let acc_id = account_id.clone().unwrap_or(env::predecessor_account_id());
        if self.data().accounts.get(&acc_id).is_none() {
            self.data_mut()
                .accounts
                .insert(acc_id.clone(), Account::default());
        }
        self.data_mut()
            .token
            .storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.data_mut().token.storage_withdraw(amount)
    }

    /// force unregister is not allowed here,
    /// return true only if both account and token.account are empty.
    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        require!(force.is_none() || !force.unwrap(), ERR_FORCE_UNGREGISTER);
        if let Some((account_id, balance)) = self.data_mut().token.internal_storage_unregister(None)
        {
            // still need to check account
            let account = self
                .data_mut()
                .accounts
                .remove(&account_id)
                .unwrap_or_default();
            require!(account.unstaked == 0, ERR_UNREGISTER_POSITIVE_UNSTAKED);
            log!("Closed @{} with {}", account_id, balance);
            true
        } else {
            false
        }
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.data().token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.data().token.storage_balance_of(account_id)
    }
}

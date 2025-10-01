use crate::*;

#[near]
impl Contract {
    #[pause]
    #[access_control_any(roles(Role::DAO))]
    #[payable]
    pub fn set_beneficiary(&mut self, account_id: AccountId, bps: u32) {
        assert_one_yocto();

        if self.data().beneficiaries.len() == MAX_BENEFICIARIES
            && self.data().beneficiaries.get(&account_id).is_none()
        {
            env::panic_str(ERR_TOO_MANY_BENEFICIARIES);
        }

        let bps_sum: u32 = self.data().beneficiaries.values().sum();

        let old_value = *self.data().beneficiaries.get(&account_id).unwrap_or(&0);

        require!(
            bps_sum - old_value + bps <= FULL_BASIS_POINTS,
            ERR_BPS_SUM_ONE
        );

        Event::SetBeneficiary {
            account_id: &account_id,
            bps: &bps,
        }
        .emit();
        self.data_mut().beneficiaries.insert(account_id, bps);
    }

    #[pause]
    #[access_control_any(roles(Role::DAO))]
    #[payable]
    pub fn remove_beneficiary(&mut self, account_id: AccountId) {
        assert_one_yocto();
        self.data_mut().beneficiaries.remove(&account_id);
        Event::RemoveBeneficiary {
            account_id: &account_id,
        }
        .emit();
    }

    /// Set whitelist account ID
    #[pause]
    #[access_control_any(roles(Role::DAO))]
    #[payable]
    pub fn set_whitelist_contract_id(&mut self, account_id: AccountId) {
        assert_one_yocto();
        self.data_mut().whitelist_account_id = Some(account_id.clone());
        Event::SetWhitelist {
            account_id: &account_id,
        }
        .emit();
    }
}

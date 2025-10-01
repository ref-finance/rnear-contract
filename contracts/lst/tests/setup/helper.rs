use crate::*;

pub const NUM_EPOCHS_TO_UNLOCK: u64 = 4;
pub const FT_STORAGE_DEPOSIT: u128 = 1_250_000_000_000_000_000_000;
/// helper functions
impl Context {
    pub async fn op_epoch_stake_all(&self) {
        let mut repeat = true;
        while repeat {
            let outcome = self.lst_contract.epoch_stake(&self.root).await.unwrap();
            assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
            // println!("logs: {:#?}", outcome.logs());
            repeat = outcome.json::<bool>().unwrap();
        }
    }

    pub async fn op_epoch_unstake_all(&self) {
        let mut repeat = true;
        while repeat {
            let outcome = self.lst_contract.epoch_unstake(&self.root).await.unwrap();
            assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
            // println!("logs: {:#?}", outcome.logs());
            repeat = outcome.json::<bool>().unwrap();
        }
    }

    pub async fn check_validator_amount(
        &self,
        validator: &MockValidatorContract,
        staked_amount: u128,
        unstaked_amount: u128,
        base_stake_amount: Option<u128>,
        target_stake_amount: Option<u128>,
    ) {
        let v_staked_amount = validator
            .get_account_staked_balance(self.lst_contract.0.id())
            .await
            .unwrap()
            .0;
        assert_eq!(v_staked_amount, staked_amount);

        let v_unstaked_amount = validator
            .get_account_unstaked_balance(self.lst_contract.0.id())
            .await
            .unwrap()
            .0;
        assert_eq!(v_unstaked_amount, unstaked_amount);

        let validator_info = self
            .lst_contract
            .get_validator(validator.0.id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(validator_info.staked_amount.0, staked_amount);
        assert_eq!(validator_info.unstaked_amount.0, unstaked_amount);

        if let Some(base_stake_amount) = base_stake_amount {
            assert_eq!(validator_info.base_stake_amount.0, base_stake_amount);
        }

        if let Some(target_stake_amount) = target_stake_amount {
            assert_eq!(validator_info.target_stake_amount.0, target_stake_amount);
        }
    }

    pub async fn epoch_height_fast_forward(&self, num_epochs: Option<u64>) {
        let cur = self.lst_contract.read_epoch_height().await.unwrap();
        check!(self
            .lst_contract
            .set_epoch_height(&self.root, cur + num_epochs.unwrap_or(NUM_EPOCHS_TO_UNLOCK)));
    }
}

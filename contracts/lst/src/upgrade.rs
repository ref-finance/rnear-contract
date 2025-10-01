use crate::*;

#[near]
impl Contract {
    /// A method to migrate a state during the contract upgrade.
    /// Can only be called after upgrade method.
    #[private]
    #[init(ignore_state)]
    pub fn migrate_state() -> Self {
        let mut contract: Contract = env::state_read().expect("ContractStateIsMissing");
        contract.data = match contract.data {
            VersionedContractData::Current(data) => VersionedContractData::Current(data),
        };
        contract
    }

    /// Returns semver of this contract.
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

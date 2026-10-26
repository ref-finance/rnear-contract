use crate::*;

pub struct Context {
    pub root: Account,
    pub manager: Account,
    pub alice: Account,
    pub bob: Account,
    pub lst_contract: LstContract,
    pub mock_whitelist: MockWhitelistContract,
}

impl Context {
    pub async fn new(worker: &Worker<Sandbox>, wasm_path: Option<&str>) -> Self {
        let root = worker.root_account().unwrap();
        let lst_wasm_path = wasm_path
            .map(str::to_owned)
            .or_else(|| std::env::var("LST_WASM_PATH").ok())
            .unwrap_or_else(|| "../../res/lst.wasm".to_string());
        let (lst_contract, mock_whitelist) = tokio::join!(
            async {
                let lst = root
                    .create_subaccount("lst")
                    .initial_balance(NearToken::from_near(100))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap();
                lst.deploy(&std::fs::read(&lst_wasm_path).unwrap())
                    .await
                    .unwrap()
                    .unwrap()
            },
            async {
                let whitelist = root
                    .create_subaccount("whitelist")
                    .initial_balance(NearToken::from_near(100))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap();
                whitelist
                    .deploy(&std::fs::read("../../res/mock_whitelist.wasm").unwrap())
                    .await
                    .unwrap()
                    .unwrap()
            },
        );
        let (manager, alice, bob) = tokio::join!(
            async {
                root.create_subaccount("manager")
                    .initial_balance(NearToken::from_near(100))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap()
            },
            async {
                root.create_subaccount("alice")
                    .initial_balance(NearToken::from_near(1000))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap()
            },
            async {
                root.create_subaccount("bob")
                    .initial_balance(NearToken::from_near(1000))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap()
            },
        );
        check!(root
            .call(lst_contract.id(), "new")
            .args_json(json!({
                "owner_id": root.id(),
            }))
            .transact());
        check!(root
            .call(mock_whitelist.id(), "new")
            .args_json(json!({}))
            .transact());
        check!(root
            .call(lst_contract.id(), "set_whitelist_contract_id")
            .args_json(json!({
                "account_id": mock_whitelist.id(),
            }))
            .deposit(NearToken::from_yoctonear(1))
            .transact());

        Self {
            root,
            manager,
            alice,
            bob,
            lst_contract: LstContract(lst_contract),
            mock_whitelist: MockWhitelistContract(mock_whitelist),
        }
    }

    pub async fn create_validator(&self, validator_id: &str) -> MockValidatorContract {
        let validator_wasm_path = std::env::var("MOCK_VALIDATOR_WASM_PATH")
            .unwrap_or_else(|_| "../../res/mock_validator.wasm".to_string());
        let (v_contract,) = tokio::join!(async {
            let v = self
                .root
                .create_subaccount(validator_id)
                .initial_balance(NearToken::from_near(20))
                .transact()
                .await
                .unwrap()
                .unwrap();
            v.deploy(&std::fs::read(&validator_wasm_path).unwrap())
                .await
                .unwrap()
                .unwrap()
        },);
        check!(self
            .root
            .call(v_contract.id(), "new")
            .args_json(json!({}))
            .transact());
        MockValidatorContract(v_contract)
    }
}

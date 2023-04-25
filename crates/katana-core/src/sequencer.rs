use anyhow::Result;

use crate::starknet::{Config, StarknetBlock, StarknetWrapper};

use blockifier::{
    abi::abi_utils::get_storage_var_address,
    state::state_api::{State, StateReader},
    transaction::{account_transaction::AccountTransaction, transactions::ExecutableTransaction},
};
use starknet::providers::jsonrpc::models::BlockId;
use starknet_api::{
    core::{calculate_contract_address, ClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
    stark_felt,
    state::StorageKey,
    transaction::{
        Calldata, ContractAddressSalt, DeployAccountTransaction, Fee, TransactionHash,
        TransactionSignature, TransactionVersion,
    },
};

pub struct KatanaSequencer {
    pub starknet: StarknetWrapper,
}

impl KatanaSequencer {
    pub fn new(origin: StarknetBlock, config: Config) -> Self {
        Self {
            starknet: StarknetWrapper::new(origin, config),
        }
    }

    pub fn drip_and_deploy_account(
        &self,
        class_hash: ClassHash,
        version: TransactionVersion,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
        balance: u64,
    ) -> anyhow::Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        self.starknet.state.lock().unwrap().set_storage_at(
            self.starknet.block_context.fee_token_address,
            deployed_account_balance_key,
            stark_felt!(balance),
        );

        self.deploy_account(
            class_hash,
            version,
            contract_address_salt,
            constructor_calldata,
            signature,
        )
    }

    pub fn deploy_account(
        &self,
        class_hash: ClassHash,
        version: TransactionVersion,
        contract_address_salt: ContractAddressSalt,
        constructor_calldata: Calldata,
        signature: TransactionSignature,
    ) -> anyhow::Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        let max_fee = self.starknet.state.lock().unwrap().get_storage_at(
            self.starknet.block_context.fee_token_address,
            account_balance_key,
        )?;

        // TODO: Compute txn hash
        let tx_hash = TransactionHash::default();
        let tx = AccountTransaction::DeployAccount(DeployAccountTransaction {
            max_fee: Fee(max_fee.try_into().unwrap()),
            version,
            class_hash,
            contract_address,
            contract_address_salt,
            constructor_calldata,
            nonce: Nonce(stark_felt!(0)),
            signature,
            transaction_hash: tx_hash,
        });
        tx.execute(
            &mut self.starknet.state.lock().unwrap(),
            &self.starknet.block_context,
        )?;

        Ok((tx_hash, contract_address))
    }

    pub async fn class_hash_at(
        &self,
        _block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, blockifier::state::errors::StateError> {
        self.starknet
            .state
            .lock()
            .unwrap()
            .get_class_hash_at(contract_address)
    }

    pub async fn get_storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<StarkFelt, blockifier::state::errors::StateError> {
        self.starknet
            .state
            .lock()
            .unwrap()
            .get_storage_at(contract_address, storage_key)
    }
}

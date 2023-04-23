use anyhow::Result;
use std::sync::Mutex;

use crate::{
    block_context::Base,
    state::DictStateReader,
    util::{
        compute_compiled_class_hash, compute_declare_hash, compute_deploy_account_transaction_hash,
        compute_legacy_declare_hash,
    },
};

use blockifier::{
    abi::abi_utils::get_storage_var_address,
    block_context::BlockContext,
    execution::contract_class::ContractClass,
    state::{
        cached_state::CachedState,
        errors::StateError,
        state_api::{State, StateReader},
    },
    transaction::{account_transaction::AccountTransaction, transactions::ExecutableTransaction},
};
use starknet::providers::jsonrpc::models::BlockId;
use starknet_api::{
    core::{calculate_contract_address, ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
    stark_felt,
    state::StorageKey,
    transaction::{
        Calldata, ContractAddressSalt, DeclareTransaction, DeclareTransactionV0V1,
        DeclareTransactionV2, DeployAccountTransaction, Fee, TransactionHash, TransactionSignature,
        TransactionVersion,
    },
};

pub struct KatanaSequencer {
    pub block_context: BlockContext,
    pub state: Mutex<CachedState<DictStateReader>>,
}

impl KatanaSequencer {
    pub fn new() -> Self {
        Self {
            block_context: BlockContext::base(),
            state: Mutex::new(CachedState::new(DictStateReader::new())),
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
    ) -> Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let deployed_account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        self.state.lock().unwrap().set_storage_at(
            self.block_context.fee_token_address,
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
    ) -> Result<(TransactionHash, ContractAddress)> {
        let contract_address = calculate_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            ContractAddress::default(),
        )
        .unwrap();

        let account_balance_key =
            get_storage_var_address("ERC20_balances", &[*contract_address.0.key()]).unwrap();
        let max_fee = self
            .state
            .lock()
            .unwrap()
            .get_storage_at(self.block_context.fee_token_address, account_balance_key)?;

        let nonce: StarkFelt = stark_felt!(0);
        let transaction_hash = compute_deploy_account_transaction_hash(
            self.block_context.chain_id.clone(),
            max_fee,
            contract_address,
            constructor_calldata.clone(),
        );
        let tx = AccountTransaction::DeployAccount(DeployAccountTransaction {
            max_fee: Fee(max_fee.try_into().unwrap()),
            version,
            class_hash,
            contract_address,
            contract_address_salt,
            constructor_calldata,
            nonce: Nonce(nonce),
            signature,
            transaction_hash,
        });
        tx.execute(&mut self.state.lock().unwrap(), &self.block_context)?;

        Ok((transaction_hash, contract_address))
    }

    pub fn declare(
        &self,
        version: u64,
        max_fee_felt: StarkFelt,
        signature: TransactionSignature,
        nonce: Nonce,
        sender_address: ContractAddress,
        contract_class: &str,
    ) -> Result<(TransactionHash, ClassHash)> {
        let max_fee_b16 = &max_fee_felt.bytes()[..16];
        let max_fee = Fee(u128::from_be_bytes(max_fee_b16.try_into().unwrap()));

        let (transaction_hash, contract_class, class_hash, declare_txn) = match version {
            0..2 => {
                let contract_class = ContractClass::default();
                let class_hash = ClassHash::default();
                let transaction_hash = compute_legacy_declare_hash(
                    self.block_context.chain_id.clone(),
                    version,
                    max_fee_felt,
                    nonce,
                    class_hash,
                    sender_address,
                );
                let declare_txn = DeclareTransaction::V0(DeclareTransactionV0V1 {
                    transaction_hash: TransactionHash::default(),
                    max_fee,
                    signature,
                    nonce,
                    class_hash,
                    sender_address,
                });

                (transaction_hash, contract_class, class_hash, declare_txn)
            }
            2 => {
                let (class_hash, contract_class, compiled_class_hash) =
                    compute_compiled_class_hash(contract_class)?;
                let transaction_hash = compute_declare_hash(
                    self.block_context.chain_id.clone(),
                    version,
                    max_fee_felt,
                    nonce,
                    class_hash,
                    compiled_class_hash,
                    sender_address,
                );
                let declare_txn = DeclareTransaction::V2(DeclareTransactionV2 {
                    transaction_hash: TransactionHash::default(),
                    max_fee,
                    signature,
                    nonce,
                    class_hash,
                    sender_address,
                    compiled_class_hash: CompiledClassHash::default(),
                });

                (transaction_hash, contract_class, class_hash, declare_txn)
            }
            _ => panic!("Unsupported declare transaction version"),
        };

        let account_tx = AccountTransaction::Declare(declare_txn, contract_class);

        // Check state before transaction application.
        if let StateError::UndeclaredClassHash(_) = self
            .state
            .lock()
            .unwrap()
            .get_contract_class(&class_hash)
            .unwrap_err()
        {}

        account_tx.execute(&mut self.state.lock().unwrap(), &self.block_context)?;

        Ok((transaction_hash, class_hash))
    }

    pub async fn class_hash_at(
        &self,
        _block_id: BlockId,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, blockifier::state::errors::StateError> {
        self.state
            .lock()
            .unwrap()
            .get_class_hash_at(contract_address)
    }

    pub async fn get_storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<StarkFelt, blockifier::state::errors::StateError> {
        self.state
            .lock()
            .unwrap()
            .get_storage_at(contract_address, storage_key)
    }
}

impl Default for KatanaSequencer {
    fn default() -> Self {
        Self::new()
    }
}

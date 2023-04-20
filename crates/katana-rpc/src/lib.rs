use blockifier::state::state_api::StateReader;
use jsonrpsee::{
    core::{async_trait, Error},
    server::{ServerBuilder, ServerHandle},
    types::error::CallError,
};
use katana_core::sequencer::KatanaSequencer;
use starknet::providers::jsonrpc::models::BlockId;
use starknet::{
    core::types::FieldElement,
    providers::jsonrpc::models::{DeclareTransactionResult, DeployTransactionResult},
};
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;
use starknet_api::{
    core::{ClassHash, ContractAddress, Nonce, PatriciaKey},
    hash::StarkFelt,
    stark_felt,
    transaction::{Calldata, ContractAddressSalt, TransactionVersion},
};
use starknet_api::{hash::StarkHash, transaction::TransactionSignature};
use std::{net::SocketAddr, sync::Arc};
use util::to_trimmed_hex_string;

use crate::api::{KatanaApiError, KatanaApiServer};
pub mod api;
mod util;

pub struct KatanaRpc {
    sequencer: Arc<KatanaSequencer>,
}

impl KatanaRpc {
    pub fn new(sequencer: KatanaSequencer) -> Self {
        Self {
            sequencer: Arc::new(sequencer),
        }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle), Error> {
        let server = ServerBuilder::new()
            .build("127.0.0.1:0")
            .await
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        let addr = server.local_addr()?;
        let handle = server.start(self.into_rpc())?;

        Ok((addr, handle))
    }
}

#[async_trait]
impl KatanaApiServer for KatanaRpc {
    async fn chain_id(&self) -> Result<String, Error> {
        Ok(self.sequencer.block_context.chain_id.as_hex())
    }

    async fn get_nonce(&self, contract_address: String) -> Result<String, Error> {
        let nonce = self
            .sequencer
            .state
            .lock()
            .unwrap()
            .get_nonce_at(ContractAddress(patricia_key!(contract_address.as_str())))
            .unwrap();

        Ok(to_trimmed_hex_string(nonce.0.bytes()))
    }

    async fn block_number(&self) -> Result<u64, Error> {
        Ok(self.sequencer.block_context.block_number.0)
    }

    async fn add_deploy_account_transaction(
        &self,
        contract_class: String,
        version: String,
        contract_address_salt: String,
        constructor_calldata: Vec<String>,
    ) -> Result<DeployTransactionResult, Error> {
        let (transaction_hash, contract_address) = self
            .sequencer
            .deploy_account(
                ClassHash(stark_felt!(contract_class.as_str())),
                TransactionVersion(stark_felt!(version.as_str())),
                ContractAddressSalt(stark_felt!(contract_address_salt.as_str())),
                Calldata(Arc::new(
                    constructor_calldata
                        .iter()
                        .map(|calldata| stark_felt!(calldata.as_str()))
                        .collect(),
                )),
                TransactionSignature::default(),
            )
            .map_err(|e| Error::Call(CallError::Failed(anyhow::anyhow!(e.to_string()))))?;

        Ok(DeployTransactionResult {
            transaction_hash: FieldElement::from_byte_slice_be(transaction_hash.0.bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
            contract_address: FieldElement::from_byte_slice_be(contract_address.0.key().bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
        })
    }

    async fn get_class_hash_at(
        &self,
        _block_id: BlockId,
        _contract_address: String,
    ) -> Result<FieldElement, Error> {
        let class_hash = self
            .sequencer
            .class_hash_at(
                starknet::providers::jsonrpc::models::BlockId::Number(0),
                ContractAddress(patricia_key!(_contract_address.as_str())),
            )
            .await
            .map_err(|_| Error::from(KatanaApiError::ContractError))
            .unwrap();
        FieldElement::from_byte_slice_be(class_hash.0.bytes())
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn get_storage_at(
        &self,
        _contract_address: String,
        _key: String,
    ) -> Result<FieldElement, Error> {
        let storage = self
            .sequencer
            .get_storage_at(
                ContractAddress(patricia_key!(_contract_address.as_str())),
                StorageKey(patricia_key!(_key.as_str())),
            )
            .await
            .map_err(|_| Error::from(KatanaApiError::ContractError))
            .unwrap();

        FieldElement::from_byte_slice_be(storage.bytes())
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn add_declare_transaction(
        &self,
        version: String,
        max_fee: String,
        signature: Vec<String>,
        nonce: String,
        contract_class: String,
        sender_address: String,
    ) -> Result<DeclareTransactionResult, Error> {
        let max_fee = stark_felt!(max_fee.as_str());
        let version: u64 = version.parse().unwrap();
        let signature =
            TransactionSignature(signature.iter().map(|s| stark_felt!(s.as_str())).collect());
        let nonce = Nonce(stark_felt!(nonce.as_str()));
        let sender_address: ContractAddress =
            ContractAddress(patricia_key!(sender_address.as_str()));

        let (transaction_hash, class_hash) = self
            .sequencer
            .declare(
                version,
                max_fee,
                signature,
                nonce,
                sender_address,
                contract_class.as_str(),
            )
            .map_err(|e| Error::Call(CallError::Failed(anyhow::anyhow!(e.to_string()))))?;

        Ok(DeclareTransactionResult {
            transaction_hash: FieldElement::from_byte_slice_be(transaction_hash.0.bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
            class_hash: FieldElement::from_byte_slice_be(class_hash.0.bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use katana_core::sequencer::KatanaSequencer;
    use starknet::core::types::FieldElement;
    use starknet_api::core::ChainId;

    use crate::{api::KatanaApiServer, KatanaRpc};

    #[tokio::test]
    async fn chain_id_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let chain_id = rpc.chain_id().await.unwrap();
        assert_eq!(chain_id, ChainId("KATANA".to_string()).as_hex());
    }

    #[tokio::test]
    async fn nonce_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let nonce = rpc.get_nonce("0xdead".to_string()).await.unwrap();
        assert_eq!(nonce, "0x0");
    }

    #[tokio::test]
    async fn block_number_is_ok() {
        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let block_number = rpc.block_number().await.unwrap();
        assert_eq!(block_number, 0);
    }

    #[tokio::test]
    async fn add_declare_transaction() {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "test-data/artifacts/erc20.sierra.json",
        ]
        .iter()
        .collect();
        let raw_contract_class = fs::read_to_string(path).unwrap();

        let rpc = KatanaRpc::new(KatanaSequencer::new());
        let result = rpc
            .add_declare_transaction(
                "2".to_string(),
                "0x0".to_string(),
                vec!["0x0".to_string()],
                "0x0".to_string(),
                raw_contract_class,
                "0xdead".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(
            result.class_hash,
            FieldElement::from_hex_be(
                "0x4bc5ed5186c60e91b8a8f0d8cdab4a4d4865d8991b6617274be7728a7f658b4"
            )
            .unwrap()
        );
    }
}

use anyhow::Result;
use blockifier::execution::contract_class::ContractClass;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet_api::{
    core::{ChainId, ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::{pedersen_hash_array, StarkFelt},
    stark_felt,
    transaction::{Calldata, TransactionHash},
};
use std::{fs, path::PathBuf};

use crate::constants::{prefix_declare, prefix_deploy_account};

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw_contract_class).unwrap()
}

pub fn compute_deploy_account_transaction_hash(
    chain_id: ChainId,
    max_fee: StarkFelt,
    contract_address: ContractAddress,
    constructor_calldata: Calldata,
) -> TransactionHash {
    let mut hash_data = vec![
        prefix_deploy_account(),
        stark_felt!(1), // version
        *contract_address.0.key(),
        stark_felt!(0), // entry_point_selector
    ];
    hash_data.extend(constructor_calldata.0.as_ref());
    hash_data.extend(vec![
        max_fee,
        stark_felt!(chain_id.as_hex().as_str()),
        stark_felt!(0),
    ]);

    TransactionHash(pedersen_hash_array(&hash_data))
}

pub fn compute_declare_hash(
    chain_id: ChainId,
    version: u64,
    max_fee: StarkFelt,
    nonce: Nonce,
    class_hash: ClassHash,
    compiled_class_hash: CompiledClassHash,
    contract_address: ContractAddress,
) -> TransactionHash {
    let hash_data = vec![
        prefix_declare(),
        stark_felt!(version), // version
        *contract_address.0.key(),
        stark_felt!(0), // entry_point_selector
        pedersen_hash_array(&[class_hash.0]),
        max_fee,
        stark_felt!(chain_id.as_hex().as_str()),
        nonce.0,
        compiled_class_hash.0,
    ];

    TransactionHash(pedersen_hash_array(&hash_data))
}

pub fn compute_legacy_declare_hash(
    chain_id: ChainId,
    version: u64,
    max_fee: StarkFelt,
    nonce: Nonce,
    class_hash: ClassHash,
    contract_address: ContractAddress,
) -> TransactionHash {
    let hash_data = vec![
        prefix_declare(),
        stark_felt!(version), // version
        *contract_address.0.key(),
        stark_felt!(0), // entry_point_selector
        pedersen_hash_array(&[class_hash.0]),
        max_fee,
        stark_felt!(chain_id.as_hex().as_str()),
        nonce.0,
    ];

    TransactionHash(pedersen_hash_array(&hash_data))
}

pub fn compute_compiled_class_hash(
    contract_class_str: &str,
) -> Result<(ClassHash, ContractClass, CompiledClassHash)> {
    let contract_class: cairo_lang_starknet::contract_class::ContractClass =
        serde_json::from_str(contract_class_str)?;
    let sierra_contract: SierraClass = ::serde_json::from_str(contract_class_str)?;
    let seirra_class_hash = sierra_contract.class_hash()?;

    let casm_contract = CasmContractClass::from_contract_class(contract_class, false)?;
    let casm_contract_str = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&casm_contract_str)?;
    let compiled_class_hash = compiled_class.class_hash()?;

    Ok((
        ClassHash(StarkFelt::from(seirra_class_hash)),
        ContractClass::try_from(casm_contract).unwrap(),
        CompiledClassHash(StarkFelt::from(compiled_class_hash)),
    ))
}

use std::{fs, path::PathBuf};

use blockifier::execution::contract_class::ContractClass;
use starknet_api::{
    core::{ChainId, ContractAddress, Nonce},
    hash::{pedersen_hash_array, StarkFelt},
    stark_felt,
    transaction::{Calldata, Fee, TransactionHash},
};

use starknet::core::types::FieldElement;

pub fn prefix_invoke() -> StarkFelt {
    StarkFelt::from(FieldElement::from_mont([
        18443034532770911073,
        18446744073709551615,
        18446744073709551615,
        513398556346534256,
    ]))
}

pub fn get_contract_class(contract_path: &str) -> ContractClass {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), contract_path].iter().collect();
    let raw_contract_class = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw_contract_class).unwrap()
}

pub fn compute_invoke_v1_transaction_hash(
    contract_address: ContractAddress,
    calldata: Calldata,
    nonce: Nonce,
    max_fee: Fee,
    chain_id: ChainId,
) -> TransactionHash {
    TransactionHash(pedersen_hash_array(&[
        prefix_invoke(), // "invoke"
        stark_felt!(1),  // version
        *contract_address.0.key(),
        stark_felt!(0), // entry_point_selector
        pedersen_hash_array(&calldata.0),
        stark_felt!(format!("{:#x}", max_fee.0).as_str()), // max_fee
        stark_felt!(chain_id.as_hex().as_str()),           // chain_id
        nonce.0,                                           // nonce
    ]))
}

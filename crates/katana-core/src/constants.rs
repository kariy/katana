use starknet::core::types::FieldElement;
use starknet_api::hash::StarkFelt;

pub fn prefix_deploy_account() -> StarkFelt {
    StarkFelt::from(FieldElement::from_mont([
        3350261884043292318,
        18443211694809419988,
        18446744073709551615,
        461298303000467581,
    ]))
}

// / Cairo string for "declare"
pub fn prefix_declare() -> StarkFelt {
    StarkFelt::from(FieldElement::from_mont([
        17542456862011667323,
        18446744073709551615,
        18446744073709551615,
        191557713328401194,
    ]))
}

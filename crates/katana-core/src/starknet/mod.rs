use std::{collections::HashMap, sync::Mutex};

use anyhow::{Ok, Result};
use blockifier::{
    block_context::BlockContext,
    state::cached_state::CachedState,
    transaction::{account_transaction::AccountTransaction, transactions::ExecutableTransaction},
};
use starknet_api::{
    block::{Block, BlockHash, BlockNumber},
    hash::StarkFelt,
    transaction::{InvokeTransaction, Transaction},
};

use crate::{block_context::Base, state::DictStateReader};

#[derive(Debug, Default, Clone)]
pub struct StarknetBlock(Block);

impl StarknetBlock {
    pub fn get_transaction_by_id(&self, transaction_id: usize) -> Option<Transaction> {
        self.0.body.transactions.get(transaction_id).cloned()
    }

    pub fn block_hash(&self) -> BlockHash {
        self.0.header.block_hash
    }

    fn compute_block_hash(&self) -> StarkFelt {
        unimplemented!("StarknetBlock::compute_block_hash")
    }
}

pub struct Config {
    blocks_on_demand: bool,
    // allow_zero_max_fee: bool,
}

pub struct StarknetBlocks {
    current_height: BlockNumber,
    pending_block: Option<StarknetBlock>,
    hash_to_num: HashMap<BlockHash, BlockNumber>,
    num_to_blocks: HashMap<BlockNumber, StarknetBlock>,
}

pub struct StarknetWrapper {
    pub config: Config,
    pub origin: StarknetBlock,
    pub blocks: StarknetBlocks,
    pub block_context: BlockContext,
    pub state: Mutex<CachedState<DictStateReader>>,
}

impl StarknetWrapper {
    pub fn new(origin: StarknetBlock, config: Config) -> Self {
        let mut hash_to_num = HashMap::new();
        let mut num_to_blocks = HashMap::new();
        num_to_blocks.insert(BlockNumber(0), origin.clone());
        hash_to_num.insert(origin.block_hash(), BlockNumber(0));

        let blocks = StarknetBlocks {
            hash_to_num,
            num_to_blocks,
            pending_block: None,
            current_height: BlockNumber(1),
        };

        let block_context = BlockContext::base();
        let state = Mutex::new(CachedState::new(DictStateReader::new()));
        Self::execute_origin_block(origin.clone(), &mut state.lock().unwrap(), &block_context);

        Self {
            state,
            config,
            blocks,
            origin,
            block_context,
        }
    }

    fn generate_pending_block(&self) {
        unimplemented!("StarknetWrapper::generate_pending_block")
    }

    // notes: transactions in origin block MUST NOT reverted.
    fn execute_origin_block(
        origin: StarknetBlock,
        state: &mut CachedState<DictStateReader>,
        block_context: &BlockContext,
    ) {
        let transactions = origin.0.body.transactions;

        for tx in transactions {
            match tx {
                Transaction::Invoke(InvokeTransaction::V1(invoke)) => {
                    let tx = AccountTransaction::Invoke(invoke);
                    tx.execute(state, block_context)
                        .unwrap_or_else(|e| panic!("{e}"));
                }
                Transaction::DeployAccount(deploy_account) => {
                    let tx = AccountTransaction::DeployAccount(deploy_account);
                    tx.execute(state, block_context)
                        .unwrap_or_else(|e| panic!("{e}"));
                }
                _ => unimplemented!("StarknetWrapper::execute_origin_block unsupported_origin_tx"),
            }
        }
    }
}

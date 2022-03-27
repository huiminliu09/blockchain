use serde::{Serialize, Deserialize};
use crate::crypto::hash::{H160, H256};
use crate::block::Block;
use crate::signedtrans::{SignedTrans};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Ping(String),
    Pong(String),
    NewBlockHashes(Vec<H256>),
    GetBlocks(Vec<H256>),
    Blocks(Vec<Block>),
    NewTransactionHashes(Vec<H256>),
    GetTransactions(Vec<H256>),
    Transactions(Vec<SignedTrans>),
    Address(Vec<H160>)
}

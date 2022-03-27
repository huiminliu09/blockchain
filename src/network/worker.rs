use super::message::Message;
use super::peer;
use crate::network::server::Handle as ServerHandle;
use crossbeam::channel;
use log::{debug, warn};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::block::Block;
use crate::crypto::hash::{H160, H256, Hashable};
use crate::blockchain::Blockchain;
use crate::signedtrans::{SignedTrans};
use crate::transaction::verify;
use crate::mempool::Mempool;

use std::thread;
use std::time::SystemTime;

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    bc: Arc<Mutex<Blockchain>>,
    mem_pool: Arc<Mutex<Mempool>>,
}

pub fn new(
    num_worker: usize,
    msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    bc: &Arc<Mutex<Blockchain>>,
    mem_pool: &Arc<Mutex<Mempool>>
) -> Context {
    Context {
        msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        bc: Arc::clone(bc),
        mem_pool: Arc::clone(mem_pool),
    }
}

impl Context {
    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&self) {

        let mut memory:HashMap<H256,Block>= HashMap::new(); // parent's hash and dangling block
        let mut total_delay:u128 = 0;
        let mut reveived:u128 = 0;

        loop {
            let msg = self.msg_chan.recv().unwrap();
            let (msg, peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                //For NewBlockHashes, if the hashes are not already in blockchain, you need to ask for them by sending GetBlocks.
                Message::NewBlockHashes(hashes) => {
                    let mut dic: HashMap<H256, u32> = HashMap::new();
                    let blkchain =self.bc.lock().unwrap();

                    for hash in hashes{
                        if !blkchain.blocks.contains_key(&hash){
                            dic.insert(hash, 1);
                        }
                    }

                    if dic.len()>0{
                        let mut new_blocks: Vec<H256>= Vec::new();
                        for item in dic {
                            new_blocks.push(item.0);
                        }
                        peer.write(Message::GetBlocks(new_blocks));
                    }
                }
                //if the hashes are in blockchain, you can get theses blocks and send them by Blocks message
                Message::GetBlocks(hashes) =>{
                    let mut dic: HashMap<H256, u32> = HashMap::new();
                    for hash in hashes{
                        dic.insert(hash, 1);
                    }
                    let mut blocks : Vec<Block> = Vec::new();
                    let blkchain =self.bc.lock().unwrap();
                    for item in dic{
                        let hash = item.0;
                        if blkchain.blocks.contains_key(&hash){
                            let temp = blkchain.blocks.get(&hash).unwrap().clone();
                            blocks.push(temp.0);
                        }
                    }
                    if blocks.len()>0{
                        peer.write(Message::Blocks(blocks));
                    }
                }
                //for Blocks, insert the blocks into blockchain if not already in it
                Message::Blocks(blocks)=>{
                    //don't find the parents of some blocks in #Block => #GetBlocks
                    //broadcast #NewBlockhashes when received onr from #Block
                    let mut dic_new: HashMap<H256, u32> = HashMap::new();
                    let mut dic_no_parent: HashMap<H256, u32> = HashMap::new();
                    let mut blkchain =self.bc.lock().unwrap();
                    let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();

                    for block in blocks.iter() {
                        if !blkchain.blocks.contains_key(&block.hash()){
                            let new_block_parent = &block.header.parent;
                            memory.insert(*new_block_parent,block.clone());
                            total_delay += ts.as_millis() - block.header.get_create_time();
                            reveived += 1;
                            // PoW validity check
                            if block.hash() <= block.header.difficulty {
                                // Parent check
                                if blkchain.blocks.contains_key(new_block_parent) {
                                    if block.header.difficulty!= blkchain.blocks.get(new_block_parent).unwrap().0.header.difficulty {
                                        continue;
                                    }
                                    let mut pool = self.mem_pool.lock().unwrap();
                                    let signed_tx = block.content.clone();
                                    for tx in signed_tx{
                                        pool.remove(&tx);
                                    }
                                    // block.hash() < blkchain.blockchain.get(new_block_parent).unwrap().header.difficulty {
                                    blkchain.insert(&block.clone());
                                    memory.remove(&block.header.parent);
                                    dic_new.insert(block.hash(), 1);

                                    // Orphan block handler: insert validated blocks stored in memory
                                    let mut inserted: H256 = block.hash();
                                    while memory.contains_key(&inserted) {
                                        let next_insert = memory.get(&inserted).unwrap().clone();
                                        let mut pool = self.mem_pool.lock().unwrap();
                                        let signed_tx = block.content.clone();
                                        for tx in signed_tx{
                                            pool.remove(&tx);
                                        }
                                        blkchain.insert(&next_insert.clone());
                                        memory.remove(&inserted);
                                        inserted = next_insert.hash();
                                        dic_new.insert(inserted, 1);
                                    }
                                } else {
                                    dic_no_parent.insert(*new_block_parent, 1);
                                }
                            }
                        }  
                    }
                    if dic_new.len()>0{
                        let mut new_hashes: Vec<H256> = Vec::new();
                        for item in dic_new {
                            new_hashes.push(item.0);
                        }
                        self.server.broadcast(Message::NewBlockHashes(new_hashes));
                    }
                    if dic_no_parent.len()>0{
                        let mut no_parents :Vec::<H256> = Vec::new();
                        for item in dic_no_parent {
                            no_parents.push(item.0);
                        }
                        peer.write(Message::GetBlocks(no_parents));
                    }
                    // if reveived>0{
                    //     println!("avg delay:{:?}/{:?}={:?}", total_delay, blkchain.get_block_num(), total_delay / reveived);
                    // }
                }

                Message::NewTransactionHashes(tx_hash) => {
                    // println!("NewTransactionHashes");
                    // println!("total block in chain {}",self.blkchain.lock().unwrap().get_num());

                    let mut new_tx_hashes:Vec<H256> = Vec::new();
                    let mem_pool = self.mem_pool.lock().unwrap();
                    for hash in tx_hash{
                        if !mem_pool.pool.contains_key(&hash){
                            new_tx_hashes.push(hash);
                        }
                    }
                    if !new_tx_hashes.is_empty(){
                        peer.write(Message::GetTransactions(new_tx_hashes));
                    }
                }

                Message::GetTransactions(tx_hash) => {
                    // println!("Received a GetTransactions message");
                    // println!("total block in chain {}",self.blkchain.lock().unwrap().get_num());

                    let mut new_tx:Vec<SignedTrans> = Vec::new();
                    let mem_pool = self.mem_pool.lock().unwrap();
                    // let pool = mem_pool.get_pool().clone();
                    for hash in tx_hash{
                        if mem_pool.pool.contains_key(&hash){
                            let signed_tx = mem_pool.pool.get(&hash).unwrap().clone();
                            new_tx.push(signed_tx);
                        }
                    }
                    if ! new_tx.is_empty(){
                        peer.write(Message::Transactions(new_tx));
                    }
                }

                Message::Transactions(txes) => {
                    println!("received Transaction: {:?} trans {:?} to {:?}",
                             H160::hash(&txes[0].public_key),
                             txes[0].transaction.outputs[0].balance,
                             txes[0].transaction.outputs[0].address);
                    // println!("total block in chain {}",self.blkchain.lock().unwrap().get_num());
                    let mem_pool = self.mem_pool.lock().unwrap().clone();
                    let mut new_tx_hashes = Vec::new();
                    let mut chain = self.bc.lock().unwrap();
                    // let mut pool = mem_pool.get_pool();
                    let pool = mem_pool.pool.clone();
                    for tx in txes{
                        if !pool.contains_key(&tx.hash()){
                            let pub_key = tx.get_public_key();
                            let trans = tx.get_tx();
                            let sig = tx.get_sig();
                            let is_verified = verify(&trans, &pub_key, &sig);
                            let is_over_spend = trans.output_val() > trans.input_val();
                            if is_verified && !(is_over_spend) {
                                let buf = tx.clone();
                                self.mem_pool.lock().unwrap().pool.insert(tx.hash(), buf);
                                new_tx_hashes.push(tx.hash());
                                chain.update_state(&tx, self.mem_pool.lock().unwrap().clone().pool.len());
                            }
                        }
                    }
                    drop(chain);
                    if !new_tx_hashes.is_empty() {
                        self.server.broadcast(Message::NewTransactionHashes(new_tx_hashes));
                    }
                }

                Message::Address(add)=>{
                    println!("new address:{:?}", add);
                    let mut blockchain = self.bc.lock().unwrap();
                    let mut newadd = vec![];
                    for address in add{
                        if !blockchain.address_list.contains(&address){
                            newadd.push(address);
                            blockchain.address_list.push(address);
                        }
                    }
                    // println!("{:?}", blockchain.address_list);
                    if newadd.len()>0{
                        for address in blockchain.address_list.clone() {
                            if !newadd.contains(&address){
                                newadd.push(address);
                            }
                        }
                        self.server.broadcast(Message::Address(newadd));
                    }
                }
            }
        }
    }
}

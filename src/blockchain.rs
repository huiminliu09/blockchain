use std::borrow::Borrow;
use std::collections::HashMap;
use std::time::SystemTime;
use crate::block::Block;
use crate::crypto::hash::{H160, H256, Hashable};
use crate::block::generate_genesis_block;
use crate::signedtrans::SignedTrans;
use crate::transaction::Transaction;
use crate::state::State;

#[derive(Debug, Clone)]
pub struct Blockchain {
    pub blockchain: HashMap<H256,Block>, //blocks in the blockchain
    pub blocks: HashMap<H256,(Block,u32)>, //all blocks in the network, u32 refers to the height of that block
    height: u32,
    tip: H256,
    block_num:u128,
    pub current_state: State,
    pub address_list: Vec<H160>
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let mut blocks = HashMap::new();
        let mut blockchain = HashMap::new();

        let genesis = generate_genesis_block(&H256::from([0u8; 32]));

        let hashvalue = genesis.hash();
        blocks.insert(hashvalue,(genesis.clone(),0));
        blockchain.insert(hashvalue,genesis.clone());
        Blockchain{
            blockchain,
            blocks,
            height: 0,
            tip: hashvalue,
            block_num: 0,
            current_state: State::new(),
            address_list: Vec::new(),
        }
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) -> u128 {
        let newblock = block.clone();
        let parent = &newblock.header.parent;
        let nheight;

        //The parent of the newly inserted block is the tip of the blockchain, insert new block directly
        if parent == &self.tip {
            self.tip = newblock.hash();
            self.height = self.height+1;
            nheight = self.height;
            self.blockchain.insert(self.tip, block.clone());
        //after insert this block, another branch becomes the longest chain
        } else if self.height < self.blocks.get(&parent).unwrap().1 +1 {
            nheight = self.blocks.get(&parent).unwrap().1 +1;
            self.height = nheight;
            //update blockchain
            let mut new_chain: Vec<H256> = Vec::new(); //the last one element's parent is in the blockchain 
            let mut current_block = &newblock;
            let mut latest_parent = &current_block.header.parent;
            while !self.blockchain.contains_key(&latest_parent){
                current_block = &self.blocks.get(&latest_parent).unwrap().0;
                latest_parent = &current_block.header.parent;  
                new_chain.push(current_block.hash()); 
            }
            //remove the blocks from blockchain
            while self.tip != self.blockchain.get(&latest_parent).unwrap().hash(){ 
                self.blockchain.remove_entry(&self.tip);
                self.tip = self.blocks.get(&self.tip).unwrap().0.header.parent; 
            }
            //insert the blocks in new_chain into blockchain
            let mut temp: Block;
            for i in new_chain.iter().rev(){ 
                temp = self.blocks.get(&i).unwrap().0.clone();
                self.blockchain.insert(*i, temp);
            }
            self.tip = newblock.hash();
            self.blockchain.insert(self.tip, block.clone());
        } else {
            //the blockchain doestn't change, only insert new block into blocks
            nheight = self.blocks.get(&parent).unwrap().1 + 1;
        }
        self.blocks.insert(newblock.hash(), (block.clone(), nheight));
        self.block_num += 1;

        let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
        println!("{:?} insert {:?} at {:?}, bc height:{:?}", ts, block.hash(), nheight, self.height);

        ts.as_millis() - block.header.get_create_time()
    }

    pub fn update_state(&mut self, sigtrans:&SignedTrans, memp_size:usize) {
        let transaction = sigtrans.clone().transaction;
        // let hash = block.hash();
        // self.block_state.insert(hash, State::new());
        // return
        let mut st = self.current_state.clone().map;
        let mut sig = self.current_state.clone().sig;
        sig.insert(sigtrans.transaction.id, sigtrans.clone());
        let mut collection = HashMap::new();
        let vec_in = transaction.inputs.clone();
        for tx_in in vec_in {
            collection.insert(tx_in.previous_hash,tx_in);
        }
        for (hash,_) in st.clone() {
            if collection.contains_key(&hash){
                st.remove(&hash);
                sig.remove(&hash);
            }
        }
        for out in transaction.clone().outputs {
            st.insert(transaction.id,out.clone());
        }

        self.current_state = State{map:st, sig};
        self.print_state(memp_size);
    }

    pub fn print_state(&self, memp_size:usize) {
        let mut balance:HashMap<H160, u8> = HashMap::new();
        for account in self.clone().address_list {
            balance.insert(account, 0);
        }
        for (_, out) in self.clone().current_state.map {
            *balance.get_mut(&out.address).unwrap() += out.balance;
        }
        println!("state:{:?} mempool size:{:?}", balance, memp_size);
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.tip
    }

    pub fn get_difficulty(&self) -> H256 {
        self.blockchain.get(self.tip.borrow()).unwrap().get_difficulty()
    }

    pub fn get_length(&self) -> u32 {
        self.height
    }

    pub fn get_block_num(&self) ->u128 {
        self.block_num
    }

    pub fn contain(&self, h:H256) -> bool {
        self.blockchain.contains_key(&h)
    }


    /// Get all blocks' hash of the longest chain
    #[cfg(any(test, test_utilities))]
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut block_hash: Vec<H256> = Vec::new();
        for block in self.blockchain.iter() {
            block_hash.push(*block.0);
        }
        block_hash
    }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::crypto::hash::Hashable;
    use crate::block::generate_random_block;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());

    }

    #[test]
      fn verify_several() {
        let mut t : HashMap<i32, i32> = HashMap::new();
        t.insert(1, 2);
        t.insert(2, 3);
        t.insert(3, 2);
        t.insert(4, 1);
        println!("----------{:?}", t.len());
        let mut i = 1;
        while t.contains_key(&i) {
            let next = t.get(&i).unwrap().clone();
            t.remove(&i);
            i = next;
        }
        println!("----------{:?}", t.len());

        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        let block2 = generate_random_block(&genesis_hash);
        let block3 = generate_random_block(&block2.hash());
        let block4 = generate_random_block(&block.hash());
        let block5 = generate_random_block(&block3.hash());
        blockchain.insert(&block);
        blockchain.insert(&block2);
        blockchain.insert(&block3);
        blockchain.insert(&block4);
        blockchain.insert(&block5);
        let result = blockchain.all_blocks_in_longest_chain();
        for i in 0..result.len() {
              println!("{}", result[i]);
            }
        assert_eq!(result, vec![ genesis_hash, block2.hash(), block3.hash(), block5.hash()]);
      }
}

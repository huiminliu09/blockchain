use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::ptr::addr_of_mut;
use crate::crypto::hash::{H256, Hashable};
use crate::signedtrans::SignedTrans;


#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Mempool {
    pub pool: HashMap<H256, SignedTrans>,
}

impl Mempool {
    pub fn new() -> Self{
        let m =Mempool {
            pool: HashMap::new()
        };
        m
    }

    pub fn add(&mut self, signed: &SignedTrans) {
        let map = self.clone().pool;
        let hash = signed.hash();
        if !map.contains_key(&hash){
            self.pool.insert(hash, signed.clone());
        };
    }

    pub fn remove(&mut self, signed: &SignedTrans) {
        let map = self.clone().pool;
        let hash = signed.hash();
        if map.contains_key(&hash) {
            self.pool.remove(&hash);
        }
        return
    }

    pub fn print(&self) {
        println!("mempool: size:{:?}", self.pool.clone().len());
    }
}
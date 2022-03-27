use serde::{Serialize, Deserialize};
use crate::transaction::{Transaction, generate_random_transaction, sign};
use crate::crypto::hash::{H256, Hashable};
use crate::crypto::key_pair;
use ring::{digest, signature::KeyPair};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTrans {
    pub transaction: Transaction,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl SignedTrans{
    pub fn get_tx(&self) -> Transaction{self.clone().transaction}
    pub fn get_sig(&self) -> Vec<u8>{self.clone().signature}
    pub fn get_public_key(&self) -> Vec<u8>{self.clone().public_key}
}

impl Hashable for SignedTrans {
    fn hash(&self) -> H256 {
        //unimplemented!()
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let mut cat = digest::Context::new(&digest::SHA256);
        cat.update(&encoded);
        let fin = cat.finish();
        let val = <H256>::from(fin);
        val
    }
}

pub fn generate_random_signedtrans() -> SignedTrans{
    let key = key_pair::random();
    let t = generate_random_transaction();
    let s = sign(&t, &key);
    let p = key.public_key().as_ref().to_vec();
    SignedTrans{
        transaction: t,
        signature: s,
        public_key: p,
    }
}

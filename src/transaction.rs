use serde::{Serialize, Deserialize};
use rand::Rng;
use std::collections::HashSet;
use ring::{digest, rand::SecureRandom, signature::Ed25519KeyPair};
use crate::crypto::hash::{H256,H160,Hashable, generate_rand_hash256,generate_rand_hash160};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Input {
    pub index: u8,
    pub previous_hash: H256,
}

impl Input{
    pub fn get_val(&self) -> u8 {self.clone().index}
    pub fn get_hash(&self) -> H256 {self.clone().previous_hash}
}

impl Hashable for Input {
    fn hash(&self) -> H256 {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let mut cat = digest::Context::new(&digest::SHA256);
        cat.update(&encoded);
        let fin = cat.finish();
        let val = <H256>::from(fin);
        val
    }
}



#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Output {
    pub balance: u8,
    pub address: H160
}

impl Output{
    pub fn get_val(&self) -> u8 {self.clone().balance }
    pub fn get_address(&self) -> H160 {self.clone().address}
}

impl Hashable for Output{
    fn hash(&self) -> H256 {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        let mut cat = digest::Context::new(&digest::SHA256);
        cat.update(&encoded);
        let fin = cat.finish();
        let val = <H256>::from(fin);
        val
    }
}



#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    pub id: H256,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>
}

impl Transaction{
    pub fn get_id(&self) -> H256{self.clone().id}
    pub fn get_input(&self) -> Vec<Input>{self.clone().inputs}
    pub fn get_output(&self) -> Vec<Output>{self.clone().outputs}

    pub fn input_hash(&self) -> HashSet<H256>{
        self.inputs.iter().map(|input|input.previous_hash).collect::<HashSet<H256>>()
    }

    pub fn output_address(&self) -> HashSet<H160>{
        self.outputs.iter().map(|output|output.address).collect::<HashSet<H160>>()
    }

    pub fn input_val(&self) -> u8 {
        self.inputs.iter().map(|input| input.index).sum()
    }

    pub fn output_val(&self) -> u8 {
        self.outputs.iter().map(|output| output.balance).sum()
    }
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Vec<u8> {
    let serialized = bincode::serialize(&t).unwrap();
    let msg = digest::digest(&digest::SHA256, &serialized);
    let sig = key.sign(msg.as_ref()).as_ref().to_vec();
    return sig;
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    let serialized = bincode::serialize(t).unwrap();
    let msg = digest::digest(&digest::SHA256, &serialized);
    let peer_public_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key.as_ref());
    peer_public_key.verify(msg.as_ref(), signature.as_ref()).is_ok()
}

pub fn coin_base(address: &H160) -> Transaction{
    use hex_literal::hex;
    let hash = (hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")).into();
    let input = Input{index: 0, previous_hash: hash};
    let output = Output{ balance: 10,  address: address.clone()};
    let t = Transaction{id:generate_rand_hash256(), inputs: vec![input], outputs: vec![output]};
    t
}

pub fn generate_random_transaction() -> Transaction {
    let sr = ring::rand::SystemRandom::new();
    let mut rng = rand::thread_rng();
    let mut result = [0u8; 32];
    sr.fill(&mut result).unwrap();
    let hash:H256 = generate_rand_hash256();
    let index:u8 = rng.gen();
    let inputs = Input{index, previous_hash:hash};
    let val:u8 = rng.gen();
    let address = generate_rand_hash160();
    let outputs = Output{ balance: val, address};
    let id = generate_rand_hash256();
    let trans = Transaction{id, inputs:vec![inputs], outputs:vec![outputs] };
    trans
}

#[cfg(any(test, test_utilities))]
mod tests {
    use ring::signature::KeyPair;
    use super::*;
    use crate::crypto::key_pair;

    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, &(key.public_key().as_ref()), &signature));
    }
}

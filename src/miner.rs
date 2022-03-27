use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use rand::Rng;
use crate::network::server::Handle as ServerHandle;
use crate::blockchain::Blockchain;
use crate::block::Block;
use crate::crypto::merkle::MerkleTree;
use crate::signedtrans::SignedTrans;
use crate::network::message::Message;
use crate::mempool::Mempool;
use crate::crypto::key_pair;


use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;

use std::thread;
use ring::signature::{Ed25519KeyPair, KeyPair};
use crate::crypto::hash::{H160, Hashable};

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    server: ServerHandle,
    bc: Arc<Mutex<Blockchain>>,
    mp: Arc<Mutex<Mempool>>,
    mined: u32,
    inserted: u32,
    start_time: SystemTime,
    key: Ed25519KeyPair,
    self_address:H160,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(
    server: &ServerHandle,
    bc: &Arc<Mutex<Blockchain>>,
    mp: &Arc<Mutex<Mempool>>
) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        bc: Arc::clone(bc),
        mp: Arc::clone(mp),
        mined: 0,
        inserted: 0,
        start_time: SystemTime::now(),
        key: key_pair::random(),
        self_address: Default::default()
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn handle_control_signal(&mut self, signal: ControlSignal) {
        match signal {
            ControlSignal::Exit => {
                info!("Miner shutting down");
                self.operating_state = OperatingState::ShutDown;
            }
            ControlSignal::Start(i) => {
                info!("Miner starting in continuous mode with lambda {}", i);
                self.start_time = SystemTime::now();
                // println!("---------- start :{:?}", SystemTime::now());
                self.operating_state = OperatingState::Run(i);
            }
        }
    }

    fn miner_loop(&mut self) {
        let mut mined_size:usize = 0;

        // main mining loop
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    self.handle_control_signal(signal);
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        self.handle_control_signal(signal);
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // get parent
            let mut bc = self.bc.lock().unwrap();
            let mp = self.mp.lock().unwrap().clone().pool;
            let parent = bc.tip();

            // get timestamp
            let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();

            // get difficulty
            let difficulty = bc.get_difficulty();

            // generate merkle root
            let mut trans = Vec::<SignedTrans>::new();
            for (_,val) in mp.clone() {
                trans.push(val);
            }
            drop(mp);
            let merkle_tree = MerkleTree::new(&trans);
            let root = merkle_tree.root();

            // generate nonce
            let nonce = rand::thread_rng().gen::<u32>();

            let blk = Block::new(parent,nonce,difficulty,timestamp,root,trans.clone());

            self.mined += 1;
            if self.mined % 1000 == 0 {
                println!("{:?} {}", difficulty, self.mined);
            }
            if blk.hash() <= difficulty && !trans.is_empty() {
                for tx in blk.clone().content {
                    self.mp.lock().unwrap().remove(&tx);
                }
                bc.insert(&blk);
                self.inserted += 1;

                // broadcast to peers
                let mut block_vec = Vec::new();
                block_vec.push(blk.hash());
                let msg = Message::NewBlockHashes(block_vec);
                self.server.broadcast(msg);

                mined_size += serde_json::to_string(&blk).unwrap().len();
                if self.inserted % 100 == 0 {
                    println!("avg block size:{:?}", mined_size as u32/self.mined);
                }
            }
            drop(bc);

            if SystemTime::now().duration_since(self.start_time).unwrap().as_secs() >= 300 {
                println!("---------- result : {:?}, {}/{}, {:?}", difficulty, self.inserted, self.mined, SystemTime::now());
                println!("========== avg block size:{:?}/{:?}={:?}", mined_size, self.inserted, mined_size as u32/self.inserted);
                break
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}

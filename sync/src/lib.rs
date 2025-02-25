// Copyright 2015, 2016 Ethcore (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

#![warn(missing_docs)]
#![cfg_attr(all(nightly, feature="dev"), feature(plugin))]
#![cfg_attr(all(nightly, feature="dev"), plugin(clippy))]

// Keeps consistency (all lines with `.clone()`) and helpful when changing ref to non-ref.
#![cfg_attr(all(nightly, feature="dev"), allow(clone_on_copy))]

//! Blockchain sync module
//! Implements ethereum protocol version 63 as specified here:
//! https://github.com/ethereum/wiki/wiki/Ethereum-Wire-Protocol
//!
//! Usage example:
//!
//! ```rust
//! extern crate ethcore_util as util;
//! extern crate ethcore;
//! extern crate ethsync;
//! use std::env;
//! use std::sync::Arc;
//! use util::network::{NetworkService, NetworkConfiguration};
//! use ethcore::client::{Client, ClientConfig};
//! use ethsync::{EthSync, SyncConfig};
//! use ethcore::ethereum;
//!
//! fn main() {
//! 	let mut service = NetworkService::start(NetworkConfiguration::new()).unwrap();
//! 	let dir = env::temp_dir();
//! 	let client = Client::new(ClientConfig::default(), ethereum::new_frontier(), &dir, service.io().channel()).unwrap();
//! 	EthSync::register(&mut service, SyncConfig::default(), client);
//! }
//! ```

#[macro_use]
extern crate log;
#[macro_use]
extern crate ethcore_util as util;
extern crate ethcore;
extern crate env_logger;
extern crate time;
extern crate rand;
extern crate rayon;
#[macro_use]
extern crate heapsize;

use std::ops::*;
use std::sync::*;
use ethcore::client::Client;
use util::network::{NetworkProtocolHandler, NetworkService, NetworkContext, PeerId};
use util::TimerToken;
use util::{U256, ONE_U256};
use chain::ChainSync;
use ethcore::service::SyncMessage;
use io::NetSyncIo;

mod chain;
mod io;
mod range_collection;
mod transaction_queue;
pub use transaction_queue::TransactionQueue;

#[cfg(test)]
mod tests;

/// Sync configuration
pub struct SyncConfig {
	/// Max blocks to download ahead
	pub max_download_ahead_blocks: usize,
	/// Network ID
	pub network_id: U256,
}

impl Default for SyncConfig {
	fn default() -> SyncConfig {
		SyncConfig {
			max_download_ahead_blocks: 20000,
			network_id: ONE_U256,
		}
	}
}

/// Current sync status
pub trait SyncProvider: Send + Sync {
	/// Get sync status
	fn status(&self) -> SyncStatus;
	/// Insert transaction in the sync transaction queue
	fn insert_transaction(&self, transaction: ethcore::transaction::SignedTransaction);
}

/// Ethereum network protocol handler
pub struct EthSync {
	/// Shared blockchain client. TODO: this should evetually become an IPC endpoint
	chain: Arc<Client>,
	/// Sync strategy
	sync: RwLock<ChainSync>
}

pub use self::chain::{SyncStatus, SyncState};

impl EthSync {
	/// Creates and register protocol with the network service
	pub fn register(service: &mut NetworkService<SyncMessage>, config: SyncConfig, chain: Arc<Client>) -> Arc<EthSync> {
		let sync = Arc::new(EthSync {
			chain: chain,
			sync: RwLock::new(ChainSync::new(config)),
		});
		service.register_protocol(sync.clone(), "eth", &[62u8, 63u8]).expect("Error registering eth protocol handler");
		sync
	}

	/// Stop sync
	pub fn stop(&mut self, io: &mut NetworkContext<SyncMessage>) {
		self.sync.write().unwrap().abort(&mut NetSyncIo::new(io, self.chain.deref()));
	}

	/// Restart sync
	pub fn restart(&mut self, io: &mut NetworkContext<SyncMessage>) {
		self.sync.write().unwrap().restart(&mut NetSyncIo::new(io, self.chain.deref()));
	}
}

impl SyncProvider for EthSync {
	/// Get sync status
	fn status(&self) -> SyncStatus {
		self.sync.read().unwrap().status()
	}

	/// Insert transaction in transaction queue
	fn insert_transaction(&self, transaction: ethcore::transaction::SignedTransaction) {
		use util::numbers::*;

		let nonce_fn = |a: &Address| self.chain.state().nonce(a) + U256::one();
		let sync = self.sync.write().unwrap();
		sync.insert_transaction(transaction, &nonce_fn);
	}
}

impl NetworkProtocolHandler<SyncMessage> for EthSync {
	fn initialize(&self, io: &NetworkContext<SyncMessage>) {
		io.register_timer(0, 1000).expect("Error registering sync timer");
	}

	fn read(&self, io: &NetworkContext<SyncMessage>, peer: &PeerId, packet_id: u8, data: &[u8]) {
		self.sync.write().unwrap().on_packet(&mut NetSyncIo::new(io, self.chain.deref()) , *peer, packet_id, data);
	}

	fn connected(&self, io: &NetworkContext<SyncMessage>, peer: &PeerId) {
		self.sync.write().unwrap().on_peer_connected(&mut NetSyncIo::new(io, self.chain.deref()), *peer);
	}

	fn disconnected(&self, io: &NetworkContext<SyncMessage>, peer: &PeerId) {
		self.sync.write().unwrap().on_peer_aborting(&mut NetSyncIo::new(io, self.chain.deref()), *peer);
	}

	fn timeout(&self, io: &NetworkContext<SyncMessage>, _timer: TimerToken) {
		self.sync.write().unwrap().maintain_peers(&mut NetSyncIo::new(io, self.chain.deref()));
		self.sync.write().unwrap().maintain_sync(&mut NetSyncIo::new(io, self.chain.deref()));
	}

	fn message(&self, io: &NetworkContext<SyncMessage>, message: &SyncMessage) {
		match *message {
			SyncMessage::NewChainBlocks { ref good, ref bad, ref retracted } => {
				let mut sync_io = NetSyncIo::new(io, self.chain.deref());
				self.sync.write().unwrap().chain_new_blocks(&mut sync_io, good, bad, retracted);
			},
			_ => {/* Ignore other messages */},
		}
	}
}

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

// Clippy config
// TODO [todr] not really sure
#![cfg_attr(all(nightly, feature="dev"), allow(needless_range_loop))]
// Shorter than if-else
#![cfg_attr(all(nightly, feature="dev"), allow(match_bool))]
// Keeps consistency (all lines with `.clone()`) and helpful when changing ref to non-ref.
#![cfg_attr(all(nightly, feature="dev"), allow(clone_on_copy))]

//! Ethcore library
//!
//! ### Rust version:
//! - nightly
//!
//! ### Supported platforms:
//! - OSX
//! - Linux
//!
//! ### Building:
//!
//! - Ubuntu 14.04 and later:
//!
//!   ```bash
//!
//!   # install multirust
//!   curl -sf https://raw.githubusercontent.com/brson/multirust/master/blastoff.sh | sh -s -- --yes
//!
//!   # export rust LIBRARY_PATH
//!   export LIBRARY_PATH=/usr/local/lib
//!
//!   # download and build parity
//!   git clone https://github.com/ethcore/parity
//!   cd parity
//!   multirust override beta
//!   cargo build --release
//!   ```
//!
//! - OSX:
//!
//!   ```bash
//!   # install rocksdb && multirust
//!   brew update
//!   brew install multirust
//!
//!   # export rust LIBRARY_PATH
//!   export LIBRARY_PATH=/usr/local/lib
//!
//!   # download and build parity
//!   git clone https://github.com/ethcore/parity
//!   cd parity
//!   multirust override beta
//!   cargo build --release
//!   ```

#[macro_use] extern crate log;
#[macro_use] extern crate ethcore_util as util;
#[macro_use] extern crate lazy_static;
extern crate rustc_serialize;
#[macro_use] extern crate heapsize;
extern crate crypto;
extern crate time;
extern crate env_logger;
extern crate num_cpus;
extern crate crossbeam;

#[cfg(test)] extern crate ethcore_devtools as devtools;
#[cfg(feature = "jit" )] extern crate evmjit;

pub mod block;
pub mod client;
pub mod error;
pub mod ethereum;
pub mod filter;
pub mod header;
pub mod service;
pub mod log_entry;
pub mod spec;
pub mod transaction;
pub mod views;
pub mod receipt;

mod common;
mod basic_types;
#[macro_use] mod evm;
mod env_info;
mod pod_account;
mod pod_state;
mod account_diff;
mod state_diff;
mod engine;
mod state;
mod account;
mod account_db;
mod action_params;
mod null_engine;
mod builtin;
mod chainfilter;
mod extras;
mod substate;
mod executive;
mod externalities;
mod verification;
mod block_queue;
mod blockchain;

#[cfg(test)]
mod tests;
#[cfg(test)]
#[cfg(feature="json-tests")]
mod json_tests;

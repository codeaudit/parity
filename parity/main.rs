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

//! Ethcore client application.

#![warn(missing_docs)]
#![cfg_attr(all(nightly, feature="dev"), feature(plugin))]
#![cfg_attr(all(nightly, feature="dev"), plugin(clippy))]
extern crate docopt;
extern crate rustc_serialize;
extern crate ethcore_util as util;
extern crate ethcore;
extern crate ethsync;
#[macro_use]
extern crate log as rlog;
extern crate env_logger;
extern crate ctrlc;
extern crate fdlimit;
extern crate daemonize;
extern crate time;
extern crate number_prefix;
extern crate rpassword;

#[cfg(feature = "rpc")]
extern crate ethcore_rpc as rpc;

use std::net::{SocketAddr};
use std::env;
use std::process::exit;
use std::path::PathBuf;
use env_logger::LogBuilder;
use ctrlc::CtrlC;
use util::*;
use util::panics::{MayPanic, ForwardPanic, PanicHandler};
use ethcore::spec::*;
use ethcore::client::*;
use ethcore::service::{ClientService, NetSyncMessage};
use ethcore::ethereum;
use ethsync::{EthSync, SyncConfig, SyncProvider};
use docopt::Docopt;
use daemonize::Daemonize;
use number_prefix::{binary_prefix, Standalone, Prefixed};

fn die_with_message(msg: &str) -> ! {
	println!("ERROR: {}", msg);
	exit(1);
}

#[macro_export]
macro_rules! die {
	($($arg:tt)*) => (die_with_message(&format!("{}", format_args!($($arg)*))));
}

const USAGE: &'static str = r#"
Parity. Ethereum Client.
  By Wood/Paronyan/Kotewicz/Drwięga/Volf.
  Copyright 2015, 2016 Ethcore (UK) Limited

Usage:
  parity daemon <pid-file> [options] [ --no-bootstrap | <enode>... ]
  parity account (new | list)
  parity [options] [ --no-bootstrap | <enode>... ]

Protocol Options:
  --chain CHAIN            Specify the blockchain type. CHAIN may be either a JSON chain specification file
                           or olympic, frontier, homestead, mainnet, morden, or testnet [default: homestead].
  --testnet                Equivalent to --chain testnet (geth-compatible).
  --networkid INDEX        Override the network identifier from the chain we are on.
  --pruning                Client should prune the state/storage trie.
  -d --datadir PATH        Specify the database & configuration directory path [default: $HOME/.parity]
  --keys-path PATH         Specify the path for JSON key files to be found [default: $HOME/.web3/keys]
  --identity NAME          Specify your node's name.

Networking Options:
  --no-bootstrap           Don't bother trying to connect to any nodes initially.
  --listen-address URL     Specify the IP/port on which to listen for peers [default: 0.0.0.0:30304].
  --public-address URL     Specify the IP/port on which peers may connect.
  --address URL            Equivalent to --listen-address URL --public-address URL.
  --peers NUM              Try to maintain that many peers [default: 25].
  --no-discovery           Disable new peer discovery.
  --no-upnp                Disable trying to figure out the correct public adderss over UPnP.
  --node-key KEY           Specify node secret key, either as 64-character hex string or input to SHA3 operation.

API and Console Options:
  -j --jsonrpc             Enable the JSON-RPC API sever.
  --jsonrpc-addr HOST      Specify the hostname portion of the JSONRPC API server [default: 127.0.0.1].
  --jsonrpc-port PORT      Specify the port portion of the JSONRPC API server [default: 8545].
  --jsonrpc-cors URL       Specify CORS header for JSON-RPC API responses [default: null].
  --jsonrpc-apis APIS      Specify the APIs available through the JSONRPC interface. APIS is a comma-delimited
                           list of API name. Possible name are web3, eth and net. [default: web3,eth,net].
  --rpc                    Equivalent to --jsonrpc (geth-compatible).
  --rpcaddr HOST           Equivalent to --jsonrpc-addr HOST (geth-compatible).
  --rpcport PORT           Equivalent to --jsonrpc-port PORT (geth-compatible).
  --rpcapi APIS            Equivalent to --jsonrpc-apis APIS (geth-compatible).
  --rpccorsdomain URL      Equivalent to --jsonrpc-cors URL (geth-compatible).

Sealing/Mining Options:
  --author ADDRESS         Specify the block author (aka "coinbase") address for sending block rewards
                           from sealed blocks [default: 0037a6b811ffeb6e072da21179d11b1406371c63].
  --extradata STRING      Specify a custom extra-data for authored blocks, no more than 32 characters.

Memory Footprint Options:
  --cache-pref-size BYTES  Specify the prefered size of the blockchain cache in bytes [default: 16384].
  --cache-max-size BYTES   Specify the maximum size of the blockchain cache in bytes [default: 262144].
  --queue-max-size BYTES   Specify the maximum size of memory to use for block queue [default: 52428800].
  --cache MEGABYTES        Set total amount of cache to use for the entire system, mutually exclusive with
                           other cache options (geth-compatible).

Miscellaneous Options:
  -l --logging LOGGING     Specify the logging level.
  -v --version             Show information about version.
  -h --help                Show this screen.
"#;

#[derive(Debug, RustcDecodable)]
struct Args {
	cmd_daemon: bool,
	cmd_account: bool,
	cmd_new: bool,
	cmd_list: bool,
	arg_pid_file: String,
	arg_enode: Vec<String>,
	flag_chain: String,
	flag_testnet: bool,
	flag_datadir: String,
	flag_networkid: Option<String>,
	flag_identity: String,
	flag_cache: Option<usize>,
	flag_keys_path: String,
	flag_pruning: bool,
	flag_no_bootstrap: bool,
	flag_listen_address: String,
	flag_public_address: Option<String>,
	flag_address: Option<String>,
	flag_peers: usize,
	flag_no_discovery: bool,
	flag_no_upnp: bool,
	flag_node_key: Option<String>,
	flag_cache_pref_size: usize,
	flag_cache_max_size: usize,
	flag_queue_max_size: usize,
	flag_jsonrpc: bool,
	flag_jsonrpc_addr: String,
	flag_jsonrpc_port: u16,
	flag_jsonrpc_cors: String,
	flag_jsonrpc_apis: String,
	flag_rpc: bool,
	flag_rpcaddr: Option<String>,
	flag_rpcport: Option<u16>,
	flag_rpccorsdomain: Option<String>,
	flag_rpcapi: Option<String>,
	flag_logging: Option<String>,
	flag_version: bool,
	flag_author: String,
	flag_extra_data: Option<String>,
}

fn setup_log(init: &Option<String>) {
	use rlog::*;

	let mut builder = LogBuilder::new();
	builder.filter(None, LogLevelFilter::Info);

	if env::var("RUST_LOG").is_ok() {
		builder.parse(&env::var("RUST_LOG").unwrap());
	}

	if let Some(ref s) = *init {
		builder.parse(s);
	}

	let format = |record: &LogRecord| {
		let timestamp = time::strftime("%Y-%m-%d %H:%M:%S %Z", &time::now()).unwrap();
		if max_log_level() <= LogLevelFilter::Info {
			format!("{}{}", timestamp, record.args())
		} else {
			format!("{}{}:{}: {}", timestamp, record.level(), record.target(), record.args())
		}
    };
	builder.format(format);
	builder.init().unwrap();
}

#[cfg(feature = "rpc")]
fn setup_rpc_server(client: Arc<Client>, sync: Arc<EthSync>, url: &str, cors_domain: &str, apis: Vec<&str>) -> Option<Arc<PanicHandler>> {
	use rpc::v1::*;

	let server = rpc::RpcServer::new();
	for api in apis.into_iter() {
		match api {
			"web3" => server.add_delegate(Web3Client::new().to_delegate()),
			"net" => server.add_delegate(NetClient::new(&sync).to_delegate()),
			"eth" => {
				server.add_delegate(EthClient::new(&client, &sync).to_delegate());
				server.add_delegate(EthFilterClient::new(&client).to_delegate());
			}
			_ => {
				die!("{}: Invalid API name to be enabled.", api);
			}
		}
	}
	Some(server.start_http(url, cors_domain, 1))
}

#[cfg(not(feature = "rpc"))]
fn setup_rpc_server(_client: Arc<Client>, _sync: Arc<EthSync>, _url: &str) -> Option<Arc<PanicHandler>> {
	None
}

fn print_version() {
	println!("\
Parity
  version {}
Copyright 2015, 2016 Ethcore (UK) Limited
License GPLv3+: GNU GPL version 3 or later <http://gnu.org/licenses/gpl.html>.
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.

By Wood/Paronyan/Kotewicz/Drwięga/Volf.\
", version());
}

struct Configuration {
	args: Args
}

impl Configuration {
	fn parse() -> Self {
		Configuration {
			args: Docopt::new(USAGE).and_then(|d| d.decode()).unwrap_or_else(|e| e.exit()),
		}
	}

	fn path(&self) -> String {
		self.args.flag_datadir.replace("$HOME", env::home_dir().unwrap().to_str().unwrap())
	}

	fn author(&self) -> Address {
		Address::from_str(&self.args.flag_author).unwrap_or_else(|_| die!("{}: Invalid address for --author. Must be 40 hex characters, without the 0x at the beginning.", self.args.flag_author))
	}

	fn extra_data(&self) -> Bytes {
		match self.args.flag_extra_data {
			Some(ref x) if x.len() <= 32 => x.as_bytes().to_owned(),
			None => version_data(),
			Some(ref x) => { die!("{}: Extra data must be at most 32 characters.", x); }
		}
	}

	fn _keys_path(&self) -> String {
		self.args.flag_keys_path.replace("$HOME", env::home_dir().unwrap().to_str().unwrap())
	}

	fn spec(&self) -> Spec {
		if self.args.flag_testnet {
			return ethereum::new_morden();
		}
		match self.args.flag_chain.as_ref() {
			"frontier" | "homestead" | "mainnet" => ethereum::new_frontier(),
			"morden" | "testnet" => ethereum::new_morden(),
			"olympic" => ethereum::new_olympic(),
			f => Spec::from_json_utf8(contents(f).unwrap_or_else(|_| die!("{}: Couldn't read chain specification file. Sure it exists?", f)).as_ref()),
		}
	}

	fn normalize_enode(e: &str) -> Option<String> {
		if is_valid_node_url(e) {
			Some(e.to_owned())
		} else {
			None
		}
	}

	fn init_nodes(&self, spec: &Spec) -> Vec<String> {
		if self.args.flag_no_bootstrap { Vec::new() } else {
			match self.args.arg_enode.len() {
				0 => spec.nodes().clone(),
				_ => self.args.arg_enode.iter().map(|s| Self::normalize_enode(s).unwrap_or_else(||die!("{}: Invalid node address format given for a boot node.", s))).collect(),
			}
		}
	}

	#[cfg_attr(all(nightly, feature="dev"), allow(useless_format))]
	fn net_addresses(&self) -> (Option<SocketAddr>, Option<SocketAddr>) {
		let mut listen_address = None;
		let mut public_address = None;

		if let Some(ref a) = self.args.flag_address {
			public_address = Some(SocketAddr::from_str(a.as_ref()).unwrap_or_else(|_| die!("{}: Invalid listen/public address given with --address", a)));
			listen_address = public_address;
		}
		if listen_address.is_none() {
			listen_address = Some(SocketAddr::from_str(self.args.flag_listen_address.as_ref()).unwrap_or_else(|_| die!("{}: Invalid listen/public address given with --listen-address", self.args.flag_listen_address)));
		}
		if let Some(ref a) = self.args.flag_public_address {
			if public_address.is_some() {
				die!("Conflicting flags provided: --address and --public-address");
			}
			public_address = Some(SocketAddr::from_str(a.as_ref()).unwrap_or_else(|_| die!("{}: Invalid listen/public address given with --public-address", a)));
		}
		(listen_address, public_address)
	}

	fn net_settings(&self, spec: &Spec) -> NetworkConfiguration {
		let mut ret = NetworkConfiguration::new();
		ret.nat_enabled = !self.args.flag_no_upnp;
		ret.boot_nodes = self.init_nodes(spec);
		let (listen, public) = self.net_addresses();
		ret.listen_address = listen;
		ret.public_address = public;
		ret.use_secret = self.args.flag_node_key.as_ref().map(|s| Secret::from_str(&s).unwrap_or_else(|_| s.sha3()));
		ret.discovery_enabled = !self.args.flag_no_discovery;
		ret.ideal_peers = self.args.flag_peers as u32;
		let mut net_path = PathBuf::from(&self.path());
		net_path.push("network");
		ret.config_path = Some(net_path.to_str().unwrap().to_owned());
		ret
	}

	fn execute(&self) {
		if self.args.flag_version {
			print_version();
			return;
		}
		if self.args.cmd_daemon {
			Daemonize::new()
				.pid_file(self.args.arg_pid_file.clone())
				.chown_pid_file(true)
				.start()
				.unwrap_or_else(|e| die!("Couldn't daemonize; {}", e));
		}
		if self.args.cmd_account {
			self.execute_account_cli();
			return;
		}
		self.execute_client();
	}

	fn execute_account_cli(&self) {
		use util::keys::store::SecretStore;
		use rpassword::read_password;
		let mut secret_store = SecretStore::new();
		if self.args.cmd_new {
			println!("Please note that password is NOT RECOVERABLE.");
			println!("Type password: ");
			let password = read_password().unwrap();
			println!("Repeat password: ");
			let password_repeat = read_password().unwrap();
			if password != password_repeat {
				println!("Passwords do not match!");
				return;
			}
			println!("New account address:");
			let new_address = secret_store.new_account(&password).unwrap();
			println!("{:?}", new_address);
			return;
		}
		if self.args.cmd_list {
			println!("Known addresses:");
			for &(addr, _) in secret_store.accounts().unwrap().iter() {
				println!("{:?}", addr);
			}
		}
	}

	fn execute_client(&self) {
		// Setup panic handler
		let panic_handler = PanicHandler::new_in_arc();

		// Setup logging
		setup_log(&self.args.flag_logging);
		// Raise fdlimit
		unsafe { ::fdlimit::raise_fd_limit(); }

		let spec = self.spec();
		let net_settings = self.net_settings(&spec);
		let mut sync_config = SyncConfig::default();
		sync_config.network_id = self.args.flag_networkid.as_ref().map(|id| U256::from_str(id).unwrap_or_else(|_| die!("{}: Invalid index given with --networkid", id))).unwrap_or(spec.network_id());

		// Build client
		let mut client_config = ClientConfig::default();
		match self.args.flag_cache {
			Some(mb) => {
				client_config.blockchain.max_cache_size = mb * 1024 * 1024;
				client_config.blockchain.pref_cache_size = client_config.blockchain.max_cache_size / 2;
			}
			None => {
				client_config.blockchain.pref_cache_size = self.args.flag_cache_pref_size;
				client_config.blockchain.max_cache_size = self.args.flag_cache_max_size;
			}
		}
		client_config.prefer_journal = self.args.flag_pruning;
		client_config.name = self.args.flag_identity.clone();
		client_config.queue.max_mem_use = self.args.flag_queue_max_size;
		let mut service = ClientService::start(client_config, spec, net_settings, &Path::new(&self.path())).unwrap();
		panic_handler.forward_from(&service);
		let client = service.client().clone();
		client.set_author(self.author());
		client.set_extra_data(self.extra_data());

		// Sync
		let sync = EthSync::register(service.network(), sync_config, client);

		// Setup rpc
		if self.args.flag_jsonrpc || self.args.flag_rpc {
			let url = format!("{}:{}",
				self.args.flag_rpcaddr.as_ref().unwrap_or(&self.args.flag_jsonrpc_addr),
				self.args.flag_rpcport.unwrap_or(self.args.flag_jsonrpc_port)
			);
			SocketAddr::from_str(&url).unwrap_or_else(|_|die!("{}: Invalid JSONRPC listen host/port given.", url));
			let cors = self.args.flag_rpccorsdomain.as_ref().unwrap_or(&self.args.flag_jsonrpc_cors);
			// TODO: use this as the API list.
			let apis = self.args.flag_rpcapi.as_ref().unwrap_or(&self.args.flag_jsonrpc_apis);
			let server_handler = setup_rpc_server(service.client(), sync.clone(), &url, cors, apis.split(",").collect());
			if let Some(handler) = server_handler {
				panic_handler.forward_from(handler.deref());
			}

		}

		// Register IO handler
		let io_handler  = Arc::new(ClientIoHandler {
			client: service.client(),
			info: Default::default(),
			sync: sync.clone(),
		});
		service.io().register_handler(io_handler).expect("Error registering IO handler");

		// Handle exit
		wait_for_exit(panic_handler);
	}
}

fn wait_for_exit(panic_handler: Arc<PanicHandler>) {
	let exit = Arc::new(Condvar::new());

	// Handle possible exits
	let e = exit.clone();
	CtrlC::set_handler(move || { e.notify_all(); });

	// Handle panics
	let e = exit.clone();
	panic_handler.on_panic(move |_reason| { e.notify_all(); });

	// Wait for signal
	let mutex = Mutex::new(());
	let _ = exit.wait(mutex.lock().unwrap()).unwrap();
}

fn main() {
	Configuration::parse().execute();
}

struct Informant {
	chain_info: RwLock<Option<BlockChainInfo>>,
	cache_info: RwLock<Option<BlockChainCacheSize>>,
	report: RwLock<Option<ClientReport>>,
}

impl Default for Informant {
	fn default() -> Self {
		Informant {
			chain_info: RwLock::new(None),
			cache_info: RwLock::new(None),
			report: RwLock::new(None),
		}
	}
}

impl Informant {
	fn format_bytes(b: usize) -> String {
		match binary_prefix(b as f64) {
			Standalone(bytes)   => format!("{} bytes", bytes),
			Prefixed(prefix, n) => format!("{:.0} {}B", n, prefix),
		}
	}

	pub fn tick(&self, client: &Client, sync: &EthSync) {
		// 5 seconds betwen calls. TODO: calculate this properly.
		let dur = 5usize;

		let chain_info = client.chain_info();
		let queue_info = client.queue_info();
		let cache_info = client.blockchain_cache_info();
		let report = client.report();
		let sync_info = sync.status();

		if let (_, _, &Some(ref last_report)) = (self.chain_info.read().unwrap().deref(), self.cache_info.read().unwrap().deref(), self.report.read().unwrap().deref()) {
			println!("[ #{} {} ]---[ {} blk/s | {} tx/s | {} gas/s  //··· {}/{} peers, #{}, {}+{} queued ···// mem: {} db, {} chain, {} queue, {} sync ]",
				chain_info.best_block_number,
				chain_info.best_block_hash,
				(report.blocks_imported - last_report.blocks_imported) / dur,
				(report.transactions_applied - last_report.transactions_applied) / dur,
				(report.gas_processed - last_report.gas_processed) / From::from(dur),

				sync_info.num_active_peers,
				sync_info.num_peers,
				sync_info.last_imported_block_number.unwrap_or(chain_info.best_block_number),
				queue_info.unverified_queue_size,
				queue_info.verified_queue_size,

				Informant::format_bytes(report.state_db_mem),
				Informant::format_bytes(cache_info.total()),
				Informant::format_bytes(queue_info.mem_used),
				Informant::format_bytes(sync_info.mem_used),
			);
		}

		*self.chain_info.write().unwrap().deref_mut() = Some(chain_info);
		*self.cache_info.write().unwrap().deref_mut() = Some(cache_info);
		*self.report.write().unwrap().deref_mut() = Some(report);
	}
}

const INFO_TIMER: TimerToken = 0;

struct ClientIoHandler {
	client: Arc<Client>,
	sync: Arc<EthSync>,
	info: Informant,
}

impl IoHandler<NetSyncMessage> for ClientIoHandler {
	fn initialize(&self, io: &IoContext<NetSyncMessage>) {
		io.register_timer(INFO_TIMER, 5000).expect("Error registering timer");
	}

	fn timeout(&self, _io: &IoContext<NetSyncMessage>, timer: TimerToken) {
		if INFO_TIMER == timer {
			self.info.tick(&self.client, &self.sync);
		}
	}
}

/// Parity needs at least 1 test to generate coverage reports correctly.
#[test]
fn if_works() {
}

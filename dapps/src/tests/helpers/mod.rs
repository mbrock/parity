// Copyright 2015-2017 Parity Technologies (UK) Ltd.
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

use std::env;
use std::str;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use env_logger::LogBuilder;
use jsonrpc_core::IoHandler;
use jsonrpc_http_server::{self as http, Host, DomainsValidation};

use devtools::http_client;
use hash_fetch::urlhint::ContractClient;
use fetch::{Fetch, Client as FetchClient};
use parity_reactor::{EventLoop, Remote};

use {Middleware, SyncStatus, WebProxyTokens};

mod registrar;
mod fetch;

use self::registrar::FakeRegistrar;
use self::fetch::FakeFetch;

const SIGNER_PORT: u16 = 18180;

fn init_logger() {
	// Initialize logger
	if let Ok(log) = env::var("RUST_LOG") {
		let mut builder = LogBuilder::new();
		builder.parse(&log);
		let _ = builder.init();	// ignore errors since ./test.sh will call this multiple times.
	}
}

pub struct ServerLoop {
	pub server: Server,
	pub event_loop: EventLoop,
}

impl ::std::ops::Deref for ServerLoop {
	type Target = Server;

	fn deref(&self) -> &Self::Target {
		&self.server
	}
}

pub fn init_server<F, B>(process: F, io: IoHandler, remote: Remote) -> (ServerLoop, Arc<FakeRegistrar>) where
	F: FnOnce(ServerBuilder) -> ServerBuilder<B>,
	B: Fetch,
{
	init_logger();
	let registrar = Arc::new(FakeRegistrar::new());
	let mut dapps_path = env::temp_dir();
	dapps_path.push("non-existent-dir-to-prevent-fs-files-from-loading");

	// TODO [ToDr] When https://github.com/paritytech/jsonrpc/issues/26 is resolved
	// this additional EventLoop wouldn't be needed, we should be able to re-use remote.
	let event_loop = EventLoop::spawn();
	let server = process(ServerBuilder::new(
		&dapps_path, registrar.clone(), remote,
	))
		.signer_address(Some(("127.0.0.1".into(), SIGNER_PORT)))
		.start_unsecured_http(&"127.0.0.1:0".parse().unwrap(), io).unwrap();
	(
		ServerLoop { server: server, event_loop: event_loop },
		registrar,
	)
}

pub fn serve_with_rpc(io: IoHandler) -> ServerLoop {
	init_server(|builder| builder, io, Remote::new_sync()).0
}

pub fn serve_hosts(hosts: Option<Vec<String>>) -> ServerLoop {
	let hosts = hosts.map(|hosts| hosts.into_iter().map(Into::into).collect());
	init_server(|builder| builder.allowed_hosts(hosts.into()), Default::default(), Remote::new_sync()).0
}

pub fn serve_with_registrar() -> (ServerLoop, Arc<FakeRegistrar>) {
	init_server(|builder| builder, Default::default(), Remote::new_sync())
}

pub fn serve_with_registrar_and_sync() -> (ServerLoop, Arc<FakeRegistrar>) {
	init_server(|builder| {
		builder.sync_status(Arc::new(|| true))
	}, Default::default(), Remote::new_sync())
}

pub fn serve_with_registrar_and_fetch() -> (ServerLoop, FakeFetch, Arc<FakeRegistrar>) {
	serve_with_registrar_and_fetch_and_threads(false)
}

pub fn serve_with_registrar_and_fetch_and_threads(multi_threaded: bool) -> (ServerLoop, FakeFetch, Arc<FakeRegistrar>) {
	let fetch = FakeFetch::default();
	let f = fetch.clone();
	let (server, reg) = init_server(move |builder| {
		builder.fetch(f.clone())
	}, Default::default(), if multi_threaded { Remote::new_thread_per_future() } else { Remote::new_sync() });

	(server, fetch, reg)
}

pub fn serve_with_fetch(web_token: &'static str) -> (ServerLoop, FakeFetch) {
	let fetch = FakeFetch::default();
	let f = fetch.clone();
	let (server, _) = init_server(move |builder| {
		builder
			.fetch(f.clone())
			.web_proxy_tokens(Arc::new(move |token| &token == web_token))
	}, Default::default(), Remote::new_sync());

	(server, fetch)
}

pub fn serve() -> ServerLoop {
	init_server(|builder| builder, Default::default(), Remote::new_sync()).0
}

pub fn request(server: ServerLoop, request: &str) -> http_client::Response {
	http_client::request(server.addr(), request)
}

pub fn assert_security_headers(headers: &[String]) {
	http_client::assert_security_headers_present(headers, None)
}
pub fn assert_security_headers_for_embed(headers: &[String]) {
	http_client::assert_security_headers_present(headers, Some(SIGNER_PORT))
}


/// Webapps HTTP+RPC server build.
pub struct ServerBuilder<T: Fetch = FetchClient> {
	dapps_path: PathBuf,
	registrar: Arc<ContractClient>,
	sync_status: Arc<SyncStatus>,
	web_proxy_tokens: Arc<WebProxyTokens>,
	signer_address: Option<(String, u16)>,
	allowed_hosts: DomainsValidation<Host>,
	remote: Remote,
	fetch: Option<T>,
}

impl ServerBuilder {
	/// Construct new dapps server
	pub fn new<P: AsRef<Path>>(dapps_path: P, registrar: Arc<ContractClient>, remote: Remote) -> Self {
		ServerBuilder {
			dapps_path: dapps_path.as_ref().to_owned(),
			registrar: registrar,
			sync_status: Arc::new(|| false),
			web_proxy_tokens: Arc::new(|_| false),
			signer_address: None,
			allowed_hosts: DomainsValidation::Disabled,
			remote: remote,
			fetch: None,
		}
	}
}

impl<T: Fetch> ServerBuilder<T> {
	/// Set a fetch client to use.
	pub fn fetch<X: Fetch>(self, fetch: X) -> ServerBuilder<X> {
		ServerBuilder {
			dapps_path: self.dapps_path,
			registrar: self.registrar,
			sync_status: self.sync_status,
			web_proxy_tokens: self.web_proxy_tokens,
			signer_address: self.signer_address,
			allowed_hosts: self.allowed_hosts,
			remote: self.remote,
			fetch: Some(fetch),
		}
	}

	/// Change default sync status.
	pub fn sync_status(mut self, status: Arc<SyncStatus>) -> Self {
		self.sync_status = status;
		self
	}

	/// Change default web proxy tokens validator.
	pub fn web_proxy_tokens(mut self, tokens: Arc<WebProxyTokens>) -> Self {
		self.web_proxy_tokens = tokens;
		self
	}

	/// Change default signer port.
	pub fn signer_address(mut self, signer_address: Option<(String, u16)>) -> Self {
		self.signer_address = signer_address;
		self
	}

	/// Change allowed hosts.
	/// `None` - All hosts are allowed
	/// `Some(whitelist)` - Allow only whitelisted hosts (+ listen address)
	pub fn allowed_hosts(mut self, allowed_hosts: DomainsValidation<Host>) -> Self {
		self.allowed_hosts = allowed_hosts;
		self
	}

	/// Asynchronously start server with no authentication,
	/// returns result with `Server` handle on success or an error.
	pub fn start_unsecured_http(self, addr: &SocketAddr, io: IoHandler) -> Result<Server, http::Error> {
		let fetch = self.fetch_client();
		Server::start_http(
			addr,
			io,
			self.allowed_hosts,
			self.signer_address,
			self.dapps_path,
			vec![],
			self.registrar,
			self.sync_status,
			self.web_proxy_tokens,
			self.remote,
			fetch,
		)
	}

	fn fetch_client(&self) -> T {
		match self.fetch.clone() {
			Some(fetch) => fetch,
			None => T::new().unwrap(),
		}
	}
}


/// Webapps HTTP server.
pub struct Server {
	server: Option<http::Server>,
}

impl Server {
	fn start_http<F: Fetch>(
		addr: &SocketAddr,
		io: IoHandler,
		allowed_hosts: DomainsValidation<Host>,
		signer_address: Option<(String, u16)>,
		dapps_path: PathBuf,
		extra_dapps: Vec<PathBuf>,
		registrar: Arc<ContractClient>,
		sync_status: Arc<SyncStatus>,
		web_proxy_tokens: Arc<WebProxyTokens>,
		remote: Remote,
		fetch: F,
	) -> Result<Server, http::Error> {
		let middleware = Middleware::new(
			remote,
			signer_address,
			dapps_path,
			extra_dapps,
			registrar,
			sync_status,
			web_proxy_tokens,
			fetch,
		);
		http::ServerBuilder::new(io)
			.request_middleware(middleware)
			.allowed_hosts(allowed_hosts)
			.cors(http::DomainsValidation::Disabled)
			.start_http(addr)
			.map(|server| Server {
				server: Some(server),
			})
	}

	/// Returns address that this server is bound to.
	pub fn addr(&self) -> &SocketAddr {
		self.server.as_ref()
			.expect("server is always Some at the start; it's consumed only when object is dropped; qed")
			.addrs()
			.first()
			.expect("You cannot start the server without binding to at least one address; qed")
	}
}

impl Drop for Server {
	fn drop(&mut self) {
		self.server.take().unwrap().close()
	}
}


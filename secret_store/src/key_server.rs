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

use std::thread;
use std::sync::Arc;
use std::sync::mpsc;
use futures::{self, Future};
use parking_lot::Mutex;
use tokio_core::reactor::Core;
use ethcrypto;
use ethkey;
use super::acl_storage::AclStorage;
use super::key_storage::KeyStorage;
use key_server_cluster::ClusterCore;
use traits::KeyServer;
use types::all::{Error, RequestSignature, DocumentAddress, DocumentEncryptedKey, DocumentEncryptedKeyShadow, ClusterConfiguration};
use key_server_cluster::{ClusterClient, ClusterConfiguration as NetClusterConfiguration};

/// Secret store key server implementation
pub struct KeyServerImpl {
	data: Arc<Mutex<KeyServerCore>>,
}

/// Secret store key server data.
pub struct KeyServerCore {
	close: Option<futures::Complete<()>>,
	handle: Option<thread::JoinHandle<()>>,
	cluster: Arc<ClusterClient>,
}

impl KeyServerImpl {
	/// Create new key server instance
	pub fn new(config: &ClusterConfiguration, acl_storage: Arc<AclStorage>, key_storage: Arc<KeyStorage>) -> Result<Self, Error> {
		Ok(KeyServerImpl {
			data: Arc::new(Mutex::new(KeyServerCore::new(config, acl_storage, key_storage)?)),
		})
	}

	#[cfg(test)]
	/// Get cluster client reference.
	pub fn cluster(&self) -> Arc<ClusterClient> {
		self.data.lock().cluster.clone()
	}
}

impl KeyServer for KeyServerImpl {
	fn generate_document_key(&self, signature: &RequestSignature, document: &DocumentAddress, threshold: usize) -> Result<DocumentEncryptedKey, Error> {
		// recover requestor' public key from signature
		let public = ethkey::recover(signature, document)
			.map_err(|_| Error::BadSignature)?;

		// generate document key
		let encryption_session = self.data.lock().cluster.new_encryption_session(document.clone(), threshold)?;
		let document_key = encryption_session.wait()?;

		// encrypt document key with requestor public key
		let document_key = ethcrypto::ecies::encrypt_single_message(&public, &document_key)
			.map_err(|err| Error::Internal(format!("Error encrypting document key: {}", err)))?;
		Ok(document_key)
	}

	fn document_key(&self, signature: &RequestSignature, document: &DocumentAddress) -> Result<DocumentEncryptedKey, Error> {
		// recover requestor' public key from signature
		let public = ethkey::recover(signature, document)
			.map_err(|_| Error::BadSignature)?;


		// decrypt document key
		let decryption_session = self.data.lock().cluster.new_decryption_session(document.clone(), signature.clone(), false)?;
		let document_key = decryption_session.wait()?.decrypted_secret;

		// encrypt document key with requestor public key
		let document_key = ethcrypto::ecies::encrypt_single_message(&public, &document_key)
			.map_err(|err| Error::Internal(format!("Error encrypting document key: {}", err)))?;
		Ok(document_key)
	}

	fn document_key_shadow(&self, signature: &RequestSignature, document: &DocumentAddress) -> Result<DocumentEncryptedKeyShadow, Error> {
		let decryption_session = self.data.lock().cluster.new_decryption_session(document.clone(), signature.clone(), false)?;
		decryption_session.wait().map_err(Into::into)
	}
}

impl KeyServerCore {
	pub fn new(config: &ClusterConfiguration, acl_storage: Arc<AclStorage>, key_storage: Arc<KeyStorage>) -> Result<Self, Error> {
		let config = NetClusterConfiguration {
			threads: config.threads,
			self_key_pair: ethkey::KeyPair::from_secret_slice(&config.self_private)?,
			listen_address: (config.listener_address.address.clone(), config.listener_address.port),
			nodes: config.nodes.iter()
				.map(|(node_id, node_address)| (node_id.clone(), (node_address.address.clone(), node_address.port)))
				.collect(),
			allow_connecting_to_higher_nodes: config.allow_connecting_to_higher_nodes,
			encryption_config: config.encryption_config.clone(),
			acl_storage: acl_storage,
			key_storage: key_storage,
		};

		let (stop, stopped) = futures::oneshot();
		let (tx, rx) = mpsc::channel();
		let handle = thread::spawn(move || {
			let mut el = match Core::new() {
				Ok(el) => el,
				Err(e) => {
					tx.send(Err(Error::Internal(format!("error initializing event loop: {}", e)))).expect("Rx is blocking upper thread.");
					return;
				},
			};
			
			let cluster = ClusterCore::new(el.handle(), config);
			let cluster_client = cluster.and_then(|c| c.run().map(|_| c.client()));
			tx.send(cluster_client.map_err(Into::into)).expect("Rx is blocking upper thread.");
			let _ = el.run(futures::empty().select(stopped));
		});
		let cluster = rx.recv().map_err(|e| Error::Internal(format!("error initializing event loop: {}", e)))??;

		Ok(KeyServerCore {
			close: Some(stop),
			handle: Some(handle),
			cluster: cluster,
		})
	}
}

impl Drop for KeyServerCore {
	fn drop(&mut self) {
		self.close.take().map(|v| v.send(()));
		self.handle.take().map(|h| h.join());
	}
}

#[cfg(test)]
mod tests {
	use std::time;
	use std::sync::Arc;
	use ethcrypto;
	use ethkey::{self, Random, Generator};
	use acl_storage::tests::DummyAclStorage;
	use key_storage::tests::DummyKeyStorage;
	use types::all::{ClusterConfiguration, NodeAddress, EncryptionConfiguration};
	use super::{KeyServer, KeyServerImpl};

	#[test]
	fn document_key_generation_and_retrievement_works_over_network() {
		//::util::log::init_log();

		let num_nodes = 3;
		let key_pairs: Vec<_> = (0..num_nodes).map(|_| Random.generate().unwrap()).collect();
		let configs: Vec<_> = (0..num_nodes).map(|i| ClusterConfiguration {
				threads: 1,
				self_private: (***key_pairs[i].secret()).into(),
				listener_address: NodeAddress {
					address: "127.0.0.1".into(),
					port: 6060 + (i as u16),
				},
				nodes: key_pairs.iter().enumerate().map(|(j, kp)| (kp.public().clone(),
					NodeAddress {
						address: "127.0.0.1".into(),
						port: 6060 + (j as u16),
					})).collect(),
				allow_connecting_to_higher_nodes: false,
				encryption_config: EncryptionConfiguration {
					key_check_timeout_ms: 10,
				},
			}).collect();
		let key_servers: Vec<_> = configs.into_iter().map(|cfg|
			KeyServerImpl::new(&cfg, Arc::new(DummyAclStorage::default()), Arc::new(DummyKeyStorage::default())).unwrap()
		).collect();

		// wait until connections are established
		let start = time::Instant::now();
		loop {
			if key_servers.iter().all(|ks| ks.cluster().cluster_state().connected.len() == num_nodes - 1) {
				break;
			}
			if time::Instant::now() - start > time::Duration::from_millis(30000) {
				panic!("connections are not established in 30000ms");
			}
		}

		let test_cases = [0, 1, 2];
		for threshold in &test_cases {
			// generate document key
			let document = Random.generate().unwrap().secret().clone();
			let secret = Random.generate().unwrap().secret().clone();
			let signature = ethkey::sign(&secret, &document).unwrap();
			let generated_key = key_servers[0].generate_document_key(&signature, &document, *threshold).unwrap();
			let generated_key = ethcrypto::ecies::decrypt_single_message(&secret, &generated_key).unwrap();

			// now let's try to retrieve key back
			for key_server in key_servers.iter() {
				let retrieved_key = key_server.document_key(&signature, &document).unwrap();
				let retrieved_key = ethcrypto::ecies::decrypt_single_message(&secret, &retrieved_key).unwrap();
				assert_eq!(retrieved_key, generated_key);
			}
		}
	}
}

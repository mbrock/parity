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

use std::io::Cursor;
use std::u16;
use std::ops::Deref;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde_json;
use ethcrypto::ecdh::agree;
use ethcrypto::ecies::{encrypt_single_message, decrypt_single_message};
use ethkey::{Public, Secret, KeyPair};
use ethkey::math::curve_order;
use util::{H256, U256};
use key_server_cluster::Error;
use key_server_cluster::message::{Message, ClusterMessage, EncryptionMessage, DecryptionMessage};

/// Size of serialized header.
pub const MESSAGE_HEADER_SIZE: usize = 4;

#[derive(Debug, PartialEq)]
/// Message header.
pub struct MessageHeader {
	/// Message/Header version.
	pub version: u8,
	/// Message kind.
	pub kind: u8,
	/// Message payload size (without header).
	pub size: u16,
}

#[derive(Debug, Clone, PartialEq)]
/// Serialized message.
pub struct SerializedMessage(Vec<u8>);

impl Deref for SerializedMessage {
	type Target = [u8];

	fn deref(&self) -> &[u8] {
		&self.0
	}
}

impl Into<Vec<u8>> for SerializedMessage {
	fn into(self) -> Vec<u8> {
		self.0
	}
}

/// Serialize message.
pub fn serialize_message(message: Message) -> Result<SerializedMessage, Error> {
	let (message_kind, payload) = match message {
		Message::Cluster(ClusterMessage::NodePublicKey(payload))							=> (1, serde_json::to_vec(&payload)),
		Message::Cluster(ClusterMessage::NodePrivateKeySignature(payload))					=> (2, serde_json::to_vec(&payload)),
		Message::Cluster(ClusterMessage::KeepAlive(payload))								=> (3, serde_json::to_vec(&payload)),
		Message::Cluster(ClusterMessage::KeepAliveResponse(payload))						=> (4, serde_json::to_vec(&payload)),

		Message::Encryption(EncryptionMessage::InitializeSession(payload))					=> (50, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::ConfirmInitialization(payload))				=> (51, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::CompleteInitialization(payload))				=> (52, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::KeysDissemination(payload))					=> (53, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::Complaint(payload))							=> (54, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::ComplaintResponse(payload))					=> (55, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::PublicKeyShare(payload))						=> (56, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::SessionError(payload))						=> (57, serde_json::to_vec(&payload)),
		Message::Encryption(EncryptionMessage::SessionCompleted(payload))					=> (58, serde_json::to_vec(&payload)),

		Message::Decryption(DecryptionMessage::InitializeDecryptionSession(payload))		=> (100, serde_json::to_vec(&payload)),
		Message::Decryption(DecryptionMessage::ConfirmDecryptionInitialization(payload))	=> (101, serde_json::to_vec(&payload)),
		Message::Decryption(DecryptionMessage::RequestPartialDecryption(payload))			=> (102, serde_json::to_vec(&payload)),
		Message::Decryption(DecryptionMessage::PartialDecryption(payload))					=> (103, serde_json::to_vec(&payload)),
		Message::Decryption(DecryptionMessage::DecryptionSessionError(payload))				=> (104, serde_json::to_vec(&payload)),
	};

	let payload = payload.map_err(|err| Error::Serde(err.to_string()))?;
	build_serialized_message(MessageHeader {
		kind: message_kind,
		version: 1,
		size: 0,
	}, payload)
}

/// Deserialize message.
pub fn deserialize_message(header: &MessageHeader, payload: Vec<u8>) -> Result<Message, Error> {
	Ok(match header.kind {
		1	=> Message::Cluster(ClusterMessage::NodePublicKey(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		2	=> Message::Cluster(ClusterMessage::NodePrivateKeySignature(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		3	=> Message::Cluster(ClusterMessage::KeepAlive(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		4	=> Message::Cluster(ClusterMessage::KeepAliveResponse(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),

		50	=> Message::Encryption(EncryptionMessage::InitializeSession(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		51	=> Message::Encryption(EncryptionMessage::ConfirmInitialization(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		52	=> Message::Encryption(EncryptionMessage::CompleteInitialization(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		53	=> Message::Encryption(EncryptionMessage::KeysDissemination(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		54	=> Message::Encryption(EncryptionMessage::Complaint(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		55	=> Message::Encryption(EncryptionMessage::ComplaintResponse(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		56	=> Message::Encryption(EncryptionMessage::PublicKeyShare(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		57	=> Message::Encryption(EncryptionMessage::SessionError(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		58	=> Message::Encryption(EncryptionMessage::SessionCompleted(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),

		100	=> Message::Decryption(DecryptionMessage::InitializeDecryptionSession(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		101	=> Message::Decryption(DecryptionMessage::ConfirmDecryptionInitialization(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		102	=> Message::Decryption(DecryptionMessage::RequestPartialDecryption(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		103	=> Message::Decryption(DecryptionMessage::PartialDecryption(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),
		104	=> Message::Decryption(DecryptionMessage::DecryptionSessionError(serde_json::from_slice(&payload).map_err(|err| Error::Serde(err.to_string()))?)),

		_ => return Err(Error::Serde(format!("unknown message type {}", header.kind))),
	})
}

/// Encrypt serialized message.
pub fn encrypt_message(key: &KeyPair, message: SerializedMessage) -> Result<SerializedMessage, Error> {
	let mut header: Vec<_> = message.into();
	let payload = header.split_off(MESSAGE_HEADER_SIZE);
	let encrypted_payload = encrypt_single_message(key.public(), &payload)?;

	let header = deserialize_header(&header)?;
	build_serialized_message(header, encrypted_payload)
}

/// Decrypt serialized message.
pub fn decrypt_message(key: &KeyPair, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
	Ok(decrypt_single_message(key.secret(), &payload)?)
}

/// Compute shared encryption key.
pub fn compute_shared_key(self_secret: &Secret, other_public: &Public) -> Result<KeyPair, Error> {
	// secret key created in agree function is invalid, as it is not calculated mod EC.field.n
	// => let's do it manually
	let shared_secret = agree(self_secret, other_public)?;
	let shared_secret: H256 = (*shared_secret).into();
	let shared_secret: U256 = shared_secret.into();
	let shared_secret: H256 = (shared_secret % curve_order()).into();
	let shared_key_pair = KeyPair::from_secret_slice(&*shared_secret)?;
	Ok(shared_key_pair)
}

/// Serialize message header.
fn serialize_header(header: &MessageHeader) -> Result<Vec<u8>, Error> {
	let mut buffer = Vec::with_capacity(MESSAGE_HEADER_SIZE);
	buffer.write_u8(header.version)?;
	buffer.write_u8(header.kind)?;
	buffer.write_u16::<LittleEndian>(header.size)?;
	Ok(buffer)
}

/// Deserialize message header.
pub fn deserialize_header(data: &[u8]) -> Result<MessageHeader, Error> {
	let mut reader = Cursor::new(data);
	Ok(MessageHeader {
		version: reader.read_u8()?,
		kind: reader.read_u8()?,
		size: reader.read_u16::<LittleEndian>()?,
	})
}

/// Build serialized message from header && payload
fn build_serialized_message(mut header: MessageHeader, payload: Vec<u8>) -> Result<SerializedMessage, Error> {
	let payload_len = payload.len();
	if payload_len > u16::MAX as usize {
		return Err(Error::InvalidMessage);
	}
	header.size = payload.len() as u16;

	let mut message = serialize_header(&header)?;
	message.extend(payload);
	Ok(SerializedMessage(message))
}

#[cfg(test)]
pub mod tests {
	use std::io;
	use futures::Poll;
	use tokio_io::{AsyncRead, AsyncWrite};
	use ethkey::{KeyPair, Public};
	use key_server_cluster::message::Message;
	use super::{MESSAGE_HEADER_SIZE, MessageHeader, compute_shared_key, encrypt_message, serialize_message,
		serialize_header, deserialize_header};

	pub struct TestIo {
		self_key_pair: KeyPair,
		peer_public: Public,
		shared_key_pair: KeyPair,
		input_buffer: io::Cursor<Vec<u8>>,
	}

	impl TestIo {
		pub fn new(self_key_pair: KeyPair, peer_public: Public) -> Self {
			let shared_key_pair = compute_shared_key(self_key_pair.secret(), &peer_public).unwrap();
			TestIo {
				self_key_pair: self_key_pair,
				peer_public: peer_public,
				shared_key_pair: shared_key_pair,
				input_buffer: io::Cursor::new(Vec::new()),
			}
		}

		pub fn self_key_pair(&self) -> &KeyPair {
			&self.self_key_pair
		}

		pub fn peer_public(&self) -> &Public {
			&self.peer_public
		}

		pub fn add_input_message(&mut self, message: Message) {
			let serialized_message = serialize_message(message).unwrap();
			let serialized_message: Vec<_> = serialized_message.into();
			let input_buffer = self.input_buffer.get_mut();
			for b in serialized_message {
				input_buffer.push(b);
			}
		}

		pub fn add_encrypted_input_message(&mut self, message: Message) {
			let serialized_message = encrypt_message(&self.shared_key_pair, serialize_message(message).unwrap()).unwrap();
			let serialized_message: Vec<_> = serialized_message.into();
			let input_buffer = self.input_buffer.get_mut();
			for b in serialized_message {
				input_buffer.push(b);
			}
		}
	}

	impl AsyncRead for TestIo {}

	impl AsyncWrite for TestIo {
		fn shutdown(&mut self) -> Poll<(), io::Error> {
			Ok(().into())
		}
	}

	impl io::Read for TestIo {
		fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
			io::Read::read(&mut self.input_buffer, buf)
		}
	}

	impl io::Write for TestIo {
		fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
			Ok(buf.len())
		}

		fn flush(&mut self) -> io::Result<()> {
			Ok(())
		}
	}

	#[test]
	fn header_serialization_works() {
		let header = MessageHeader {
			kind: 1,
			version: 2,
			size: 3,
		};

		let serialized_header = serialize_header(&header).unwrap();
		assert_eq!(serialized_header.len(), MESSAGE_HEADER_SIZE);

		let deserialized_header = deserialize_header(&serialized_header).unwrap();
		assert_eq!(deserialized_header, header);
	}
}

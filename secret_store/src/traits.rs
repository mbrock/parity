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

use types::all::{Error, RequestSignature, DocumentAddress, DocumentEncryptedKey, DocumentEncryptedKeyShadow};

#[ipc(client_ident="RemoteKeyServer")]
/// Secret store key server
pub trait KeyServer: Send + Sync {
	/// Generate encryption key for given document.
	fn generate_document_key(&self, signature: &RequestSignature, document: &DocumentAddress, threshold: usize) -> Result<DocumentEncryptedKey, Error>;
	/// Request encryption key of given document for given requestor
	fn document_key(&self, signature: &RequestSignature, document: &DocumentAddress) -> Result<DocumentEncryptedKey, Error>;
	/// Request encryption key of given document for given requestor.
	/// This method does not reveal document_key to any KeyServer, but it requires additional actions on client.
	/// To calculate decrypted key on client:
	/// 1) use requestor secret key to decrypt secret coefficients from result.decrypt_shadows
	/// 2) calculate decrypt_shadows_sum = sum of all secrets from (1)
	/// 3) calculate decrypt_shadow_point: decrypt_shadows_sum * result.common_point
	/// 4) calculate decrypted_secret: result.decrypted_secret + decrypt_shadow_point
	fn document_key_shadow(&self, signature: &RequestSignature, document: &DocumentAddress) -> Result<DocumentEncryptedKeyShadow, Error>;
}

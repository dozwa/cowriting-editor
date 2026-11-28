//! Messages to the editor's own server are sealed before they leave the computer: encrypted with a
//! public key built in here, so that only the server, which holds the private key, can read them.
//! HTTPS protects a message on the way; this also keeps it from whatever ends HTTPS in front of
//! the server (a proxy such as Cloudflare).
//!
//! The scheme is libsodium's sealed box (X25519, XSalsa20-Poly1305), nothing of our own. Before
//! sealing, the message's JSON is filled up with spaces to one of a few fixed sizes, so that a
//! sealed message's length says little about what is in it. What goes out is
//!
//! ```json
//! { "v": 1, "key": "<key id>", "sealed": "<base64>" }
//! ```
//!
//! Sealing hides what a message says. It does not hide that one was sent, when, or from which
//! internet address, and it proves nothing about who sent it.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use crypto_box::aead::OsRng;
use crypto_box::PublicKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// A public key of the server, and the name it has in a sealed message.
pub struct Key {
    /// The first 16 hex digits of the SHA-256 of the public key.
    pub id: &'static str,
    /// The key's 32 bytes in base64.
    pub public: &'static str,
}

/// The key this version seals for. A new key comes with a new version; the server goes on
/// opening messages for the old one as long as older versions may send.
pub const KEY: Key = Key { id: "d3a8fed2bc7c849a", public: "MTNrLMUGRoJp20HDXh5iTqjjqknNLaQuySi7jNTC7E4=" };

/// The sizes a message is filled up to before it is sealed, in bytes.
pub const SIZES: [usize; 4] = [512, 1024, 2048, 4096];

/// The name a public key has in a sealed message.
pub fn key_id(public: &PublicKey) -> String {
    Sha256::digest(public.as_bytes()).iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

impl Key {
    pub fn public_key(&self) -> Result<PublicKey, String> {
        let bytes = BASE64.decode(self.public).map_err(|_| "The built-in key is not a key.")?;
        PublicKey::from_slice(&bytes).map_err(|_| "The built-in key is not a key.".to_string())
    }
}

/// Seals a message for a public key. A message longer than the longest fixed size is not sealed, and so not sent.
pub fn seal(message: &Value, public: &PublicKey) -> Result<Value, String> {
    let mut filled = message.to_string().into_bytes();
    let size = SIZES.into_iter().find(|size| filled.len() <= *size).ok_or("The message is too long to be sealed.")?;
    filled.resize(size, b' ');
    let sealed = public.seal(&mut OsRng, &filled).map_err(|_| "The message could not be sealed.")?;
    Ok(json!({ "v": 1, "key": key_id(public), "sealed": BASE64.encode(sealed) }))
}

/*
 * Delta API
 *
 * Copyright 2024 Maxim Menshikov
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the “Software”),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

//! Control-plane command signing (server side of `delta --verify-key`).
//!
//! The `delta` agent, when run with `--verify-key <hex>`, acts only on control
//! commands wrapped in an Ed25519 **signed envelope**:
//!
//! ```json
//! { "signed": "<hex of the exact command-JSON bytes>",
//!   "sig":    "<hex Ed25519 signature over those bytes>" }
//! ```
//!
//! and drops anything at or below the last accepted `seq` (replay protection).
//! This module is the counterpart that *produces* those envelopes: generate a
//! keypair, hand out the public key (passed to the agent as `--verify-key`),
//! and sign command payloads. It is byte-compatible with the agent's TweetNaCl
//! verifier and with `tools/delta-sign.py` — the 32-byte secret is the Ed25519
//! seed, and the signed bytes are exactly what rides in `signed`.

use ed25519_dalek::{Signer, SigningKey};

/// A control-plane signing key (Ed25519). Holds the secret; the public half is
/// what the fleet pins via `--verify-key`.
pub struct ControlKey {
    signing: SigningKey,
}

/// Errors from loading a key or parsing hex.
#[derive(Debug, PartialEq, Eq)]
pub enum ControlKeyError {
    /// Hex string had the wrong length or a non-hex digit.
    BadHex,
}

fn hex_encode(bytes: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(D[(b >> 4) as usize] as char);
        s.push(D[(b & 0xf) as usize] as char);
    }
    s
}

fn hex_decode(s: &str, out: &mut [u8]) -> Result<(), ControlKeyError> {
    let bytes = s.as_bytes();
    if bytes.len() != out.len() * 2 {
        return Err(ControlKeyError::BadHex);
    }
    let nib = |c: u8| -> Result<u8, ControlKeyError> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(ControlKeyError::BadHex),
        }
    };
    for (i, o) in out.iter_mut().enumerate() {
        *o = (nib(bytes[i * 2])? << 4) | nib(bytes[i * 2 + 1])?;
    }
    Ok(())
}

impl ControlKey {
    /// Generate a fresh key from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("OS CSPRNG unavailable");
        Self {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    /// Load from a 64-hex-char secret **seed** (as emitted by `seed_hex` and by
    /// `delta-sign.py keygen`'s `.sk` file).
    pub fn from_seed_hex(hex: &str) -> Result<Self, ControlKeyError> {
        let mut seed = [0u8; 32];
        hex_decode(hex.trim(), &mut seed)?;
        Ok(Self {
            signing: SigningKey::from_bytes(&seed),
        })
    }

    /// The secret seed as hex — persist this server-side, never on a device.
    pub fn seed_hex(&self) -> String {
        hex_encode(&self.signing.to_bytes())
    }

    /// The public key as hex — this is the `--verify-key` value for the agent.
    pub fn public_hex(&self) -> String {
        hex_encode(self.signing.verifying_key().as_bytes())
    }

    /// Sign raw payload bytes, returning the envelope JSON the agent accepts.
    /// The caller is responsible for the payload being valid command JSON;
    /// `sign_command` is the convenience wrapper that builds and seqs it.
    pub fn envelope_for_bytes(&self, payload: &[u8]) -> String {
        let sig = self.signing.sign(payload).to_bytes();
        format!(
            "{{\"signed\":\"{}\",\"sig\":\"{}\"}}",
            hex_encode(payload),
            hex_encode(&sig)
        )
    }

    /// Wrap a command payload in a signed envelope, injecting `seq` for replay
    /// protection when given. `commands` is the value of the `commands` array;
    /// the signed payload is `{"commands":[...],"seq":N}`.
    pub fn sign_command(&self, commands: &serde_json::Value, seq: Option<u64>) -> String {
        let mut obj = serde_json::Map::new();
        obj.insert("commands".to_string(), commands.clone());
        if let Some(n) = seq {
            obj.insert("seq".to_string(), serde_json::json!(n));
        }
        // Compact serialization; the exact bytes are what we sign and hex-carry,
        // and the agent re-parses them, so key ordering is irrelevant to
        // validity.
        let payload = serde_json::to_vec(&serde_json::Value::Object(obj))
            .expect("serializing a JSON object cannot fail");
        self.envelope_for_bytes(&payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-answer vector produced by tools/delta-sign.py with the fixed seed
    /// 01,02,…,20. Ed25519 is deterministic, so the Rust signer must reproduce
    /// the exact public key and signature the C agent (TweetNaCl) verifies.
    const SEED_HEX: &str =
        "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    const PUB_HEX: &str =
        "79b5562e8fe654f94078b112e8a98ba7901f853ae695bed7e0e3910bad049664";
    // payload bytes: {"commands":[{"action":"disable"}]}
    const PAYLOAD: &str = "{\"commands\":[{\"action\":\"disable\"}]}";
    const SIG_HEX: &str =
        "d447fec51131a185847e26d49327a0fb52ff3695575faff28f34d3eefde7fb48\
         de9504dba41c913a8e26f0affdb31c5b72d0b56effdd9b3e600a03000b5a1d0c";

    #[test]
    fn derives_public_key_from_seed() {
        let k = ControlKey::from_seed_hex(SEED_HEX).unwrap();
        assert_eq!(k.public_hex(), PUB_HEX);
        assert_eq!(k.seed_hex(), SEED_HEX);
    }

    #[test]
    fn signature_matches_reference_signer() {
        let k = ControlKey::from_seed_hex(SEED_HEX).unwrap();
        let env = k.envelope_for_bytes(PAYLOAD.as_bytes());
        let expected = format!(
            "{{\"signed\":\"{}\",\"sig\":\"{}\"}}",
            hex_encode(PAYLOAD.as_bytes()),
            SIG_HEX.replace(['\n', ' '], "")
        );
        assert_eq!(env, expected);
    }

    #[test]
    fn sign_command_injects_seq_and_round_trips() {
        let k = ControlKey::generate();
        let cmds = serde_json::json!([{ "action": "disable" }]);
        let env = k.sign_command(&cmds, Some(7));

        // Envelope parses, "signed" decodes, and the payload carries seq:7.
        let v: serde_json::Value = serde_json::from_str(&env).unwrap();
        let signed = v["signed"].as_str().unwrap();
        let mut bytes = vec![0u8; signed.len() / 2];
        hex_decode(signed, &mut bytes).unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["seq"], 7);
        assert_eq!(payload["commands"][0]["action"], "disable");

        // And the signature verifies under the public key (self-consistency).
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let mut pk = [0u8; 32];
        hex_decode(&k.public_hex(), &mut pk).unwrap();
        let vk = VerifyingKey::from_bytes(&pk).unwrap();
        let mut sig = [0u8; 64];
        hex_decode(v["sig"].as_str().unwrap(), &mut sig).unwrap();
        assert!(vk.verify(&bytes, &Signature::from_bytes(&sig)).is_ok());
    }

    #[test]
    fn rejects_bad_hex() {
        // Non-hex digits and wrong length both fail closed.
        assert!(matches!(
            ControlKey::from_seed_hex(&"z".repeat(64)),
            Err(ControlKeyError::BadHex)
        ));
        assert!(matches!(
            ControlKey::from_seed_hex("00"),
            Err(ControlKeyError::BadHex)
        ));
    }
}

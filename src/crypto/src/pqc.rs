use pqcrypto_kyber::*;
use pqcrypto_dilithium::*;
use pqcrypto_traits::kem::Ciphertext;
use pqcrypto_traits::sign::DetachedSignature;

/// Post-quantum cryptography module for Conduwuit
/// Uses CRYSTALS-Kyber for key encapsulation and CRYSTALS-Dilithium for signatures
pub struct PQCryptoManager {
    kyber_keypair: Option<(kyber768::PublicKey, kyber768::SecretKey)>,
    dilithium_keypair: Option<(dilithium3::PublicKey, dilithium3::SecretKey)>,
}

impl PQCryptoManager {
    pub fn new() -> Self {
        Self {
            kyber_keypair: None,
            dilithium_keypair: None,
        }
    }

    /// Generate new Kyber-768 keypair
    pub fn generate_kyber_keypair(&mut self) {
        let (pk, sk) = kyber768::keypair();
        self.kyber_keypair = Some((pk, sk));
    }

    /// Generate new Dilithium3 keypair
    pub fn generate_dilithium_keypair(&mut self) {
        let (pk, sk) = dilithium3::keypair();
        self.dilithium_keypair = Some((pk, sk));
    }

    /// Encapsulate a shared secret using Kyber-768
    pub fn encapsulate(&self, peer_public_key: &kyber768::PublicKey) -> (Vec<u8>, kyber768::SharedSecret) {
        let (shared_secret, ciphertext) = kyber768::encapsulate(peer_public_key);
        // Convert ciphertext to bytes
        let mut ciphertext_bytes = Vec::new();
        ciphertext_bytes.extend_from_slice(ciphertext.as_bytes());
        (ciphertext_bytes, shared_secret)
    }

    /// Decapsulate a shared secret using Kyber-768
    pub fn decapsulate(&self, ciphertext: &[u8]) -> Option<kyber768::SharedSecret> {
        if let Some((_, sk)) = &self.kyber_keypair {
            // Create a new Ciphertext from bytes
            let ct = kyber768::Ciphertext::from_bytes(ciphertext).ok()?;
            Some(kyber768::decapsulate(&ct, sk))
        } else {
            None
        }
    }

    /// Sign a message using Dilithium3
    pub fn sign(&self, message: &[u8]) -> Option<Vec<u8>> {
        if let Some((_, sk)) = &self.dilithium_keypair {
            let signature = dilithium3::detached_sign(message, sk);
            // Convert signature to bytes
            let mut signature_bytes = Vec::new();
            signature_bytes.extend_from_slice(signature.as_bytes());
            Some(signature_bytes)
        } else {
            None
        }
    }

    /// Verify a signature using Dilithium3
    pub fn verify(&self, message: &[u8], signature_bytes: &[u8], public_key: &dilithium3::PublicKey) -> bool {
        // Convert bytes to DetachedSignature
        if let Ok(signature) = dilithium3::DetachedSignature::from_bytes(signature_bytes) {
            dilithium3::verify_detached_signature(&signature, message, public_key).is_ok()
        } else {
            false
        }
    }

    /// Get the Kyber public key
    pub fn get_kyber_public_key(&self) -> Option<&kyber768::PublicKey> {
        self.kyber_keypair.as_ref().map(|(pk, _)| pk)
    }

    /// Get the Dilithium public key
    pub fn get_dilithium_public_key(&self) -> Option<&dilithium3::PublicKey> {
        self.dilithium_keypair.as_ref().map(|(pk, _)| pk)
    }
}

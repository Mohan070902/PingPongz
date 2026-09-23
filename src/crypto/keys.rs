use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

pub type Fingerprint = String;

pub struct CryptoKeys {
    pub ed25519_signing_key: SigningKey,
    pub ed25519_verifying_key: VerifyingKey,
    pub x25519_secret: StaticSecret,
    pub x25519_public: X25519PublicKey,
    pub fingerprint: Fingerprint,
}

impl CryptoKeys {
    /// FR-ID-02: Generate one Ed25519 key pair and one X25519 key pair
    pub fn generate_new() -> Self {
        let mut rng = OsRng;
        let ed25519_signing_key = SigningKey::generate(&mut rng);
        let ed25519_verifying_key = ed25519_signing_key.verifying_key();

        let x25519_secret = StaticSecret::random_from_rng(&mut rng);
        let x25519_public = X25519PublicKey::from(&x25519_secret);

        let fingerprint = Self::derive_fingerprint(&ed25519_verifying_key);

        Self {
            ed25519_signing_key,
            ed25519_verifying_key,
            x25519_secret,
            x25519_public,
            fingerprint,
        }
    }

    /// FR-ID-04: Derive fingerprint as first 12 hex characters of Ed25519 public key
    pub fn derive_fingerprint(verifying_key: &VerifyingKey) -> Fingerprint {
        let pub_hex = hex::encode(verifying_key.as_bytes());
        pub_hex[0..12].to_uppercase()
    }

    /// Derive fingerprint directly from 32-byte Ed25519 public key bytes
    pub fn fingerprint_from_bytes(bytes: &[u8; 32]) -> Fingerprint {
        let pub_hex = hex::encode(bytes);
        pub_hex[0..12].to_uppercase()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.ed25519_signing_key.sign(message)
    }

    pub fn verify(
        verifying_key: &VerifyingKey,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), ed25519_dalek::SignatureError> {
        verifying_key.verify(message, signature)
    }

    pub fn ed25519_pubkey_hex(&self) -> String {
        hex::encode(self.ed25519_verifying_key.as_bytes())
    }

    pub fn x25519_pubkey_hex(&self) -> String {
        hex::encode(self.x25519_public.as_bytes())
    }

    pub fn ed25519_privkey_hex(&self) -> String {
        hex::encode(self.ed25519_signing_key.to_bytes())
    }

    pub fn x25519_privkey_hex(&self) -> String {
        hex::encode(self.x25519_secret.to_bytes())
    }

    pub fn from_hex(
        ed25519_priv_hex: &str,
        x25519_priv_hex: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let ed_bytes = hex::decode(ed25519_priv_hex)?;
        if ed_bytes.len() != 32 {
            return Err("Invalid Ed25519 private key length".into());
        }
        let mut ed_arr = [0u8; 32];
        ed_arr.copy_from_slice(&ed_bytes);
        let ed25519_signing_key = SigningKey::from_bytes(&ed_arr);
        let ed25519_verifying_key = ed25519_signing_key.verifying_key();

        let x_bytes = hex::decode(x25519_priv_hex)?;
        if x_bytes.len() != 32 {
            return Err("Invalid X25519 private key length".into());
        }
        let mut x_arr = [0u8; 32];
        x_arr.copy_from_slice(&x_bytes);
        let x25519_secret = StaticSecret::from(x_arr);
        let x25519_public = X25519PublicKey::from(&x25519_secret);

        let fingerprint = Self::derive_fingerprint(&ed25519_verifying_key);

        Ok(Self {
            ed25519_signing_key,
            ed25519_verifying_key,
            x25519_secret,
            x25519_public,
            fingerprint,
        })
    }
}

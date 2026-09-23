use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};
use tracing::debug;

pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
pub const NOISE_MAX_FRAME_SIZE: usize = 65535;

pub struct NoiseSession {
    pub transport: TransportState,
    pub remote_static_key: Vec<u8>,
}

impl NoiseSession {
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0u8; plaintext.len() + 16];
        let len = self.transport.write_message(plaintext, &mut out)?;
        out.truncate(len);
        Ok(out)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0u8; ciphertext.len()];
        let len = self.transport.read_message(ciphertext, &mut out)?;
        out.truncate(len);
        Ok(out)
    }
}

pub struct NoiseHandshake {
    state: HandshakeState,
    pub is_initiator: bool,
}

impl NoiseHandshake {
    pub fn new_initiator(x25519_privkey: &[u8; 32]) -> Result<Self, snow::Error> {
        let params: NoiseParams = NOISE_PATTERN.parse()?;
        let state = Builder::new(params)
            .local_private_key(x25519_privkey)
            .build_initiator()?;

        Ok(Self {
            state,
            is_initiator: true,
        })
    }

    pub fn new_responder(x25519_privkey: &[u8; 32]) -> Result<Self, snow::Error> {
        let params: NoiseParams = NOISE_PATTERN.parse()?;
        let state = Builder::new(params)
            .local_private_key(x25519_privkey)
            .build_responder()?;

        Ok(Self {
            state,
            is_initiator: false,
        })
    }

    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0u8; NOISE_MAX_FRAME_SIZE];
        let len = self.state.write_message(payload, &mut out)?;
        out.truncate(len);
        debug!("Noise handshake write: {} bytes", len);
        Ok(out)
    }

    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut payload = vec![0u8; NOISE_MAX_FRAME_SIZE];
        let len = self.state.read_message(message, &mut payload)?;
        payload.truncate(len);
        debug!("Noise handshake read: {} bytes payload", len);
        Ok(payload)
    }

    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    pub fn into_session(self) -> Result<NoiseSession, snow::Error> {
        let remote_static = self
            .state
            .get_remote_static()
            .map(|k| k.to_vec())
            .unwrap_or_default();
        let transport = self.state.into_transport_mode()?;
        Ok(NoiseSession {
            transport,
            remote_static_key: remote_static,
        })
    }
}

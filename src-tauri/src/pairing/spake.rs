use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::error::{AppError, AppResult};


pub struct PairingExchange {
    state: Spake2<Ed25519Group>,
}


pub struct SharedSecret(pub Vec<u8>);

impl PairingExchange {


    pub fn start(confirm_code: &str, session_identity: &str) -> (Self, Vec<u8>) {
        let (state, outbound) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(confirm_code.as_bytes()),
            &Identity::new(session_identity.as_bytes()),
        );
        (Self { state }, outbound)
    }


    pub fn finish(self, inbound: &[u8]) -> AppResult<SharedSecret> {
        self.state
            .finish(inbound)
            .map(SharedSecret)
            .map_err(|e| AppError::new(format!("SPAKE2 exchange failed: {e}")))
    }
}

impl SharedSecret {


    pub fn debug_fingerprint(&self) -> String {
        blake3::hash(&self.0).to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_codes_derive_matching_secrets() {
        let (a, msg_a) = PairingExchange::start("7F3-KX9", "session-123");
        let (b, msg_b) = PairingExchange::start("7F3-KX9", "session-123");

        let secret_a = a.finish(&msg_b).unwrap();
        let secret_b = b.finish(&msg_a).unwrap();

        assert_eq!(secret_a.0, secret_b.0);
    }

    #[test]
    fn mismatched_codes_derive_different_secrets() {
        let (a, msg_a) = PairingExchange::start("7F3-KX9", "session-123");
        let (b, msg_b) = PairingExchange::start("AAA-AAA", "session-123");

        let secret_a = a.finish(&msg_b).unwrap();
        let secret_b = b.finish(&msg_a).unwrap();

        assert_ne!(secret_a.0, secret_b.0);
    }
}

use crate::crypto::CryptoState;
use std::sync::Arc;

pub struct Services {
    crypto_state: Arc<CryptoState>,
    // ... other services
}

impl Services {
    pub fn new() -> Self {
        Self {
            crypto_state: Arc::new(CryptoState::new()),
            // ... initialize other services
        }
    }

    pub fn crypto_state(&self) -> Arc<CryptoState> {
        self.crypto_state.clone()
    }
}

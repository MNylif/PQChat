mod pqc;

pub use pqc::PQCryptoManager;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Global cryptographic state manager
pub struct CryptoState {
    pqc: Arc<RwLock<PQCryptoManager>>,
}

impl CryptoState {
    pub fn new() -> Self {
        let mut pqc = PQCryptoManager::new();
        pqc.generate_kyber_keypair();
        pqc.generate_dilithium_keypair();
        
        Self {
            pqc: Arc::new(RwLock::new(pqc)),
        }
    }

    pub async fn get_pqc(&self) -> tokio::sync::RwLockReadGuard<'_, PQCryptoManager> {
        self.pqc.read().await
    }

    pub async fn get_pqc_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, PQCryptoManager> {
        self.pqc.write().await
    }
}

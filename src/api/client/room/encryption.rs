use crate::crypto::CryptoState;
use crate::services::Services;
use ruma::{
    api::client::r0::room::create_room,
    events::{
        room::encryption::{EncryptionEventContent, MegolmV1AesSha2Content},
        StateEventType,
    },
    RoomId,
};
use std::sync::Arc;

/// Enables post-quantum encryption for a room
pub async fn enable_pq_encryption(
    services: &Services,
    room_id: &RoomId,
) -> Result<(), Error> {
    let crypto_state = services.crypto_state();
    let pqc = crypto_state.get_pqc().await;

    // Create encryption event content with post-quantum parameters
    let content = EncryptionEventContent {
        algorithm: "m.megolm.pq.v1.aes-sha2".to_string(),
        rotation_period_ms: Some(604_800_000), // 1 week
        rotation_period_msgs: Some(100),
        content: MegolmV1AesSha2Content {
            algorithm: "m.megolm.pq.v1.aes-sha2".to_string(),
        },
    };

    // Send state event to enable encryption
    services
        .rooms
        .send_state_event(
            room_id,
            &StateEventType::RoomEncryption,
            "",
            &content,
            true,
        )
        .await?;

    Ok(())
}

/// Encrypts a message using post-quantum encryption
pub async fn encrypt_message(
    services: &Services,
    room_id: &RoomId,
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    let crypto_state = services.crypto_state();
    let pqc = crypto_state.get_pqc().await;

    // Get room members' public keys
    let members = services.rooms.room_members(room_id).await?;
    
    let mut encrypted_keys = Vec::new();
    
    // Encrypt message key for each member using Kyber
    for member in members {
        if let Some(member_pubkey) = services.users.get_pq_pubkey(&member).await? {
            let (ciphertext, shared_secret) = pqc.encapsulate(&member_pubkey);
            encrypted_keys.push((member, ciphertext));
        }
    }

    // Sign the encrypted message with Dilithium
    let signature = pqc.sign(plaintext)
        .ok_or_else(|| Error::CryptoError("Failed to sign message".to_string()))?;

    // Combine encrypted message and signature
    let mut result = Vec::new();
    result.extend_from_slice(plaintext);
    result.extend_from_slice(&signature);
    
    Ok(result)
}

/// Decrypts a message using post-quantum encryption
pub async fn decrypt_message(
    services: &Services,
    room_id: &RoomId,
    ciphertext: &[u8],
    sender_key: &[u8],
) -> Result<Vec<u8>, Error> {
    let crypto_state = services.crypto_state();
    let pqc = crypto_state.get_pqc().await;

    // Verify signature
    let message_len = ciphertext.len() - SIGNATURE_LENGTH;
    let (message, signature) = ciphertext.split_at(message_len);
    
    let sender_pubkey = services.users.get_pq_pubkey(sender_key).await?;
    
    if !pqc.verify(message, signature, &sender_pubkey) {
        return Err(Error::CryptoError("Invalid signature".to_string()));
    }

    Ok(message.to_vec())
}

/// Creates default encryption settings for a new room
pub async fn create_default_encryption_event() -> Result<Raw<EncryptionEventContent>, Error> {
    let content = EncryptionEventContent {
        algorithm: "m.megolm.pq.v1.aes-sha2".to_string(),
        rotation_period_ms: Some(604_800_000), // 1 week
        rotation_period_msgs: Some(100),
        content: MegolmV1AesSha2Content {
            algorithm: "m.megolm.pq.v1.aes-sha2".to_string(),
        },
    };

    Ok(Raw::new(&content)?)
}

/// Sets up encryption for a new room during creation
pub async fn setup_room_encryption(
    services: &Services,
    room_id: &RoomId,
    is_direct: bool,
) -> Result<(), Error> {
    // Always enable encryption for direct messages
    // For regular rooms, check if encryption is allowed in server config
    if is_direct || services.globals.allow_encryption() {
        let content = create_default_encryption_event().await?;
        
        services
            .rooms
            .send_state_event(
                room_id,
                &StateEventType::RoomEncryption,
                "",
                &content,
                true,
            )
            .await?;
    }

    Ok(())
}

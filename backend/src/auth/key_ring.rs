use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::Context;
use jsonwebtoken::{DecodingKey, EncodingKey};
use rand::RngCore;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct SigningKeyEntry {
    pub kid: String,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

pub struct KeyRing {
    active: RwLock<Option<SigningKeyEntry>>,
    pool: PgPool,
    encryption_secret: [u8; 32],
}

impl KeyRing {
    pub fn new(pool: PgPool, hex_secret: &str) -> anyhow::Result<Arc<Self>> {
        let bytes = hex::decode(hex_secret).context("KEY_ENCRYPTION_SECRET must be valid hex")?;
        anyhow::ensure!(
            bytes.len() == 32,
            "KEY_ENCRYPTION_SECRET must be 32 bytes (64 hex chars)"
        );
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        Ok(Arc::new(Self {
            active: RwLock::new(None),
            pool,
            encryption_secret: secret,
        }))
    }

    pub async fn load_or_generate(&self) -> anyhow::Result<()> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT kid, public_key, private_key FROM signing_keys \
             WHERE retired_at IS NULL ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some((kid, public_pem, encrypted_private)) = row {
            let private_pem = self.decrypt_pem(&encrypted_private)?;
            let entry = SigningKeyEntry {
                kid: kid.clone(),
                encoding_key: EncodingKey::from_rsa_pem(private_pem.as_bytes())
                    .context("failed to load encoding key")?,
                decoding_key: DecodingKey::from_rsa_pem(public_pem.as_bytes())
                    .context("failed to load decoding key")?,
            };
            *self.active.write().await = Some(entry);
            info!(kid, "loaded existing signing key");
        } else {
            self.generate_and_store().await?;
        }
        Ok(())
    }

    async fn generate_and_store(&self) -> anyhow::Result<()> {
        info!("generating new RSA-2048 signing key");
        let (private_pem, public_pem) =
            tokio::task::spawn_blocking(generate_rsa_keypair).await??;
        let kid = Uuid::new_v4().to_string();
        let encrypted_private = self.encrypt_pem(&private_pem)?;

        sqlx::query(
            "INSERT INTO signing_keys (kid, public_key, private_key) VALUES ($1, $2, $3)",
        )
        .bind(&kid)
        .bind(&public_pem)
        .bind(&encrypted_private)
        .execute(&self.pool)
        .await?;

        let entry = SigningKeyEntry {
            kid: kid.clone(),
            encoding_key: EncodingKey::from_rsa_pem(private_pem.as_bytes())?,
            decoding_key: DecodingKey::from_rsa_pem(public_pem.as_bytes())?,
        };
        *self.active.write().await = Some(entry);
        info!(kid, "generated and stored new signing key");
        Ok(())
    }

    pub async fn active_key(&self) -> Option<SigningKeyEntry> {
        self.active.read().await.clone()
    }

    pub async fn decoding_key_for(&self, _kid: &str) -> Option<DecodingKey> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|k| k.decoding_key.clone())
    }

    fn encrypt_pem(&self, pem: &str) -> anyhow::Result<String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_secret));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt failed: {e}"))?;
        Ok(format!(
            "{}:{}",
            hex::encode(nonce_bytes),
            hex::encode(ciphertext)
        ))
    }

    fn decrypt_pem(&self, encrypted: &str) -> anyhow::Result<String> {
        let (nonce_hex, ct_hex) = encrypted
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid encrypted key format"))?;
        let nonce_bytes = hex::decode(nonce_hex)?;
        let ciphertext = hex::decode(ct_hex)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.encryption_secret));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|e| anyhow::anyhow!("decrypt failed: {e}"))?;
        Ok(String::from_utf8(plaintext)?)
    }
}

fn generate_rsa_keypair() -> anyhow::Result<(String, String)> {
    use rsa::{
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
        RsaPrivateKey,
    };
    let mut rng = rand::thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
    let public_key = private_key.to_public_key();
    let private_pem = private_key.to_pkcs8_pem(LineEnding::LF)?.to_string();
    let public_pem = public_key.to_public_key_pem(LineEnding::LF)?;
    Ok((private_pem, public_pem))
}

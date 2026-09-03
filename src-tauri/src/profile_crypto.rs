use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const ENVELOPE_FORMAT: &str = "tabularis-profile-aes256gcm-v1";
const PROFILE_KEY_BYTES: usize = 32;
const PROFILE_KEY_ENTRY: &str = "profile-encryption-key-v1";
const AUDIT_PSEUDONYM_ENTRY: &str = "audit-pseudonym-key-v1";

static PROFILE_KEY: OnceLock<[u8; PROFILE_KEY_BYTES]> = OnceLock::new();
static AUDIT_PSEUDONYM_KEY: OnceLock<[u8; PROFILE_KEY_BYTES]> = OnceLock::new();
static KEY_GATE: Mutex<()> = Mutex::new(());
static FILE_GATE: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedEnvelope {
    format: String,
    purpose: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug)]
pub struct ProfileRead<T> {
    pub value: T,
    pub was_plaintext: bool,
    pub plaintext_bytes: Vec<u8>,
}

fn decode_key(value: &str) -> Result<[u8; PROFILE_KEY_BYTES], String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| "The profile key stored in the OS keychain is invalid".to_string())?;
    bytes
        .try_into()
        .map_err(|_| "The profile key stored in the OS keychain has an invalid length".to_string())
}

fn encode_key(key: &[u8; PROFILE_KEY_BYTES]) -> String {
    base64::engine::general_purpose::STANDARD.encode(key)
}

fn get_or_create_key(
    cache: &'static OnceLock<[u8; PROFILE_KEY_BYTES]>,
    entry_name: &str,
) -> Result<[u8; PROFILE_KEY_BYTES], String> {
    if let Some(key) = cache.get() {
        return Ok(*key);
    }

    let _guard = KEY_GATE.lock().map_err(|error| error.to_string())?;
    if let Some(key) = cache.get() {
        return Ok(*key);
    }

    let key = match crate::keychain_utils::get_private_value(entry_name)? {
        Some(value) => decode_key(&value)?,
        None => {
            let key = Aes256Gcm::generate_key(&mut OsRng);
            let key: [u8; PROFILE_KEY_BYTES] = key.into();
            crate::keychain_utils::set_private_value(entry_name, &encode_key(&key))?;
            key
        }
    };
    let _ = cache.set(key);
    Ok(key)
}

fn profile_key() -> Result<[u8; PROFILE_KEY_BYTES], String> {
    get_or_create_key(&PROFILE_KEY, PROFILE_KEY_ENTRY)
}

fn audit_pseudonym_key() -> Result<[u8; PROFILE_KEY_BYTES], String> {
    get_or_create_key(&AUDIT_PSEUDONYM_KEY, AUDIT_PSEUDONYM_ENTRY)
}

fn additional_data(purpose: &str) -> Vec<u8> {
    format!("{ENVELOPE_FORMAT}:{purpose}").into_bytes()
}

#[cfg(test)]
fn encrypt_bytes_with_key(
    plaintext: &[u8],
    purpose: &str,
    key: &[u8; PROFILE_KEY_BYTES],
) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let payload = aes_gcm::aead::Payload {
        msg: plaintext,
        aad: &additional_data(purpose),
    };
    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|_| "Failed to encrypt the local profile".to_string())?;
    let envelope = EncryptedEnvelope {
        format: ENVELOPE_FORMAT.to_string(),
        purpose: purpose.to_string(),
        nonce: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    };
    serde_json::to_vec_pretty(&envelope).map_err(|error| error.to_string())
}

fn decrypt_bytes_with_key(
    envelope: &EncryptedEnvelope,
    purpose: &str,
    key: &[u8; PROFILE_KEY_BYTES],
) -> Result<Vec<u8>, String> {
    if envelope.format != ENVELOPE_FORMAT || envelope.purpose != purpose {
        return Err("The encrypted profile type does not match this file".to_string());
    }
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(&envelope.nonce)
        .map_err(|_| "The encrypted profile nonce is invalid".to_string())?;
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| "The encrypted profile nonce has an invalid length".to_string())?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|_| "The encrypted profile payload is invalid".to_string())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
    cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            aes_gcm::aead::Payload {
                msg: &ciphertext,
                aad: &additional_data(purpose),
            },
        )
        .map_err(|_| "The local profile could not be decrypted for this OS user".to_string())
}

pub fn managed_profile_path(path: &Path) -> bool {
    #[cfg(test)]
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.contains(".managed-test."))
    {
        return true;
    }
    path.parent()
        .is_some_and(|parent| parent == crate::paths::get_app_config_dir())
}

fn envelope_from_bytes(bytes: &[u8]) -> Option<EncryptedEnvelope> {
    let envelope = serde_json::from_slice::<EncryptedEnvelope>(bytes).ok()?;
    (envelope.format == ENVELOPE_FORMAT).then_some(envelope)
}

pub fn file_is_encrypted(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| envelope_from_bytes(&bytes))
        .is_some()
}

fn write_and_sync(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Profile path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Profile file name is invalid".to_string())?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4().simple()));
    write_and_sync(&temporary, bytes)?;

    if !path.exists() {
        return fs::rename(&temporary, path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            error.to_string()
        });
    }

    let previous = parent.join(format!(".{file_name}.previous"));
    if previous.exists() {
        fs::remove_file(&previous).map_err(|error| error.to_string())?;
    }
    fs::rename(path, &previous).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&previous, path);
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

fn decode_json<T: DeserializeOwned>(
    raw: Vec<u8>,
    purpose: &str,
    key: &[u8; PROFILE_KEY_BYTES],
) -> Result<ProfileRead<T>, String> {
    let (plaintext, was_plaintext) = match envelope_from_bytes(&raw) {
        Some(envelope) => (decrypt_bytes_with_key(&envelope, purpose, key)?, false),
        None => (raw, true),
    };
    let value = serde_json::from_slice(&plaintext).map_err(|error| error.to_string())?;
    Ok(ProfileRead {
        value,
        was_plaintext,
        plaintext_bytes: plaintext,
    })
}

pub fn read_json<T: DeserializeOwned>(
    path: &Path,
    purpose: &str,
) -> Result<Option<ProfileRead<T>>, String> {
    let _guard = FILE_GATE.lock().map_err(|error| error.to_string())?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read(path).map_err(|error| error.to_string())?;
    let primary_key = if envelope_from_bytes(&raw).is_some() {
        profile_key()?
    } else {
        [0; PROFILE_KEY_BYTES]
    };
    match decode_json(raw, purpose, &primary_key) {
        Ok(profile) => Ok(Some(profile)),
        Err(primary_error) => {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("profile");
            let previous = path.with_file_name(format!(".{file_name}.previous"));
            if !previous.exists() {
                return Err(primary_error);
            }
            let previous_raw = fs::read(previous).map_err(|_| primary_error.clone())?;
            let previous_key = if envelope_from_bytes(&previous_raw).is_some() {
                profile_key()?
            } else {
                [0; PROFILE_KEY_BYTES]
            };
            let recovered =
                decode_json(previous_raw.clone(), purpose, &previous_key).or_else(|_| {
                    let envelope =
                        envelope_from_bytes(&previous_raw).ok_or_else(|| primary_error.clone())?;
                    let plaintext = decrypt_bytes_with_key(
                        &envelope,
                        &format!("legacy-backup:{purpose}"),
                        &previous_key,
                    )?;
                    let value =
                        serde_json::from_slice(&plaintext).map_err(|_| primary_error.clone())?;
                    Ok::<ProfileRead<T>, String>(ProfileRead {
                        value,
                        was_plaintext: false,
                        plaintext_bytes: plaintext,
                    })
                })?;
            eprintln!("Recovered a local profile from its atomic rollback copy");
            Ok(Some(recovered))
        }
    }
}

pub fn read_legacy_backup<T: DeserializeOwned>(
    path: &Path,
    purpose: &str,
) -> Result<Option<T>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Profile path has no parent directory".to_string())?;
    let backup_directory = parent.join("migration-backups");
    if !backup_directory.exists() {
        return Ok(None);
    }
    let file_prefix = format!(
        "{}-legacy-",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("profile")
    );
    let mut backups = fs::read_dir(backup_directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(&file_prefix) && name.ends_with(".enc"))
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    for backup in backups {
        let raw = fs::read(backup.path()).map_err(|error| error.to_string())?;
        let Some(envelope) = envelope_from_bytes(&raw) else {
            continue;
        };
        let plaintext = decrypt_bytes_with_key(
            &envelope,
            &format!("legacy-backup:{purpose}"),
            &profile_key()?,
        )?;
        let value = serde_json::from_slice(&plaintext).map_err(|error| error.to_string())?;
        return Ok(Some(value));
    }
    Ok(None)
}

/// Local profiles are stored as plain JSON. Encryption at rest was removed
/// (this fork's repository is already scrubbed); `read_json` still decrypts
/// envelopes written by earlier builds so existing profiles migrate back to
/// plaintext on first load. `purpose` is kept for signature compatibility.
pub fn write_json<T: Serialize>(path: &Path, _purpose: &str, value: &T) -> Result<(), String> {
    let plaintext = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let _guard = FILE_GATE.lock().map_err(|error| error.to_string())?;
    atomic_replace(path, &plaintext)
}

pub fn pseudonymize(label: &str, value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Ok(String::new());
    }
    let key = audit_pseudonym_key()?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&key).map_err(|error| error.to_string())?;
    mac.update(label.as_bytes());
    mac.update(&[0]);
    mac.update(value.as_bytes());
    let digest = hex::encode(mac.finalize().into_bytes());
    Ok(format!("{label}_{}", &digest[..16]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; PROFILE_KEY_BYTES] = [0x5a; PROFILE_KEY_BYTES];

    #[test]
    fn envelope_round_trip_is_purpose_bound_and_hides_plaintext() {
        let plaintext = br#"[{"host":"db.example.invalid","password":"secret"}]"#;
        let encrypted = encrypt_bytes_with_key(plaintext, "connections", &TEST_KEY).unwrap();
        assert!(!encrypted
            .windows("secret".len())
            .any(|part| part == b"secret"));
        let envelope = envelope_from_bytes(&encrypted).unwrap();
        assert_eq!(
            decrypt_bytes_with_key(&envelope, "connections", &TEST_KEY).unwrap(),
            plaintext
        );
        assert!(decrypt_bytes_with_key(&envelope, "ssh-connections", &TEST_KEY).is_err());
    }

    #[test]
    fn changed_ciphertext_is_rejected() {
        let encrypted = encrypt_bytes_with_key(b"profile", "connections", &TEST_KEY).unwrap();
        let mut envelope = envelope_from_bytes(&encrypted).unwrap();
        envelope.ciphertext.push('A');
        assert!(decrypt_bytes_with_key(&envelope, "connections", &TEST_KEY).is_err());
    }

    #[test]
    fn unmanaged_test_paths_remain_plain_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.json");
        write_json(&path, "connections", &serde_json::json!({"value": 7})).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"value\""));
        let loaded = read_json::<serde_json::Value>(&path, "connections")
            .unwrap()
            .unwrap();
        assert!(loaded.was_plaintext);
        assert_eq!(loaded.value["value"], 7);
    }
}

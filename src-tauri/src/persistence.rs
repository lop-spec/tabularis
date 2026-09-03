use crate::models::{ConnectionGroup, ConnectionsFile, SavedConnection, SshConnection};
use crate::{keychain_utils, profile_crypto};
use std::fs;
use std::path::Path;

const CONNECTIONS_PURPOSE: &str = "database-connections";
const SSH_CONNECTIONS_PURPOSE: &str = "ssh-connections";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalProfileSummary {
    pub database_connections: usize,
    pub ssh_connections: usize,
}

fn parse_connections(value: serde_json::Value) -> Result<ConnectionsFile, String> {
    if value.is_array() {
        let connections: Vec<SavedConnection> =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        Ok(ConnectionsFile {
            connections,
            groups: Vec::new(),
        })
    } else {
        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

fn protect_connection_secrets(connection: &mut SavedConnection) -> Result<bool, String> {
    let mut changed = false;
    if connection.params.audit_profile.is_none() {
        if let Some(profile) = crate::audit_outbox::audit_profile_from_name(&connection.name) {
            connection.params.audit_profile = Some(profile.to_string());
            changed = true;
        }
    }
    let id = connection.id.as_str();
    let params = &mut connection.params;

    if let Some(uri) = params
        .connection_uri
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        keychain_utils::set_connection_uri(id, uri)?;
        params.connection_uri_in_keychain = Some(true);
        changed = true;
    }

    if !params.use_iam_auth.unwrap_or(false) {
        if let Some(password) = params.password.as_deref().filter(|value| !value.is_empty()) {
            keychain_utils::set_db_password(id, password)?;
            changed = true;
        }
    }
    if let Some(password) = params
        .ssh_password
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        keychain_utils::set_ssh_password(id, password)?;
        changed = true;
    }
    if let Some(passphrase) = params
        .ssh_key_passphrase
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        keychain_utils::set_ssh_key_passphrase(id, passphrase)?;
        changed = true;
    }

    changed |= params.password.is_some()
        || params.connection_uri.is_some()
        || params.ssh_password.is_some()
        || params.ssh_key_passphrase.is_some()
        || params.save_in_keychain != Some(true);
    params.password = None;
    params.connection_uri = None;
    params.ssh_password = None;
    params.ssh_key_passphrase = None;
    params.save_in_keychain = Some(true);
    Ok(changed)
}

fn protect_file_secrets(file: &mut ConnectionsFile) -> Result<bool, String> {
    file.connections
        .iter_mut()
        .try_fold(false, |changed, connection| {
            protect_connection_secrets(connection)
                .map(|connection_changed| changed || connection_changed)
        })
}

fn merge_connection_recovery(current: &mut ConnectionsFile, backup: ConnectionsFile) -> bool {
    let mut changed = false;
    let mut connection_ids = current
        .connections
        .iter()
        .map(|connection| connection.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for connection in backup.connections {
        if connection_ids.insert(connection.id.clone()) {
            current.connections.push(connection);
            changed = true;
        }
    }
    let mut group_ids = current
        .groups
        .iter()
        .map(|group| group.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for group in backup.groups {
        if group_ids.insert(group.id.clone()) {
            current.groups.push(group);
            changed = true;
        }
    }
    changed
}

pub fn load_connections_file(path: &Path) -> Result<ConnectionsFile, String> {
    if !path.exists() {
        return Ok(ConnectionsFile::default());
    }

    if profile_crypto::managed_profile_path(path) || profile_crypto::file_is_encrypted(path) {
        let read = profile_crypto::read_json::<serde_json::Value>(path, CONNECTIONS_PURPOSE)?
            .ok_or_else(|| "Connections profile disappeared while it was being read".to_string())?;
        let mut file = parse_connections(read.value)?;
        // Encrypted profiles from earlier builds are converted back to plain
        // JSON on first load. Entries that only survive in the legacy backup
        // are merged in during that one-time conversion.
        let was_encrypted = !read.was_plaintext;
        let recovered = if was_encrypted {
            profile_crypto::read_legacy_backup::<serde_json::Value>(path, CONNECTIONS_PURPOSE)?
                .map(parse_connections)
                .transpose()?
                .is_some_and(|backup| merge_connection_recovery(&mut file, backup))
        } else {
            false
        };
        let changed = protect_file_secrets(&mut file)? || recovered;
        if was_encrypted || changed {
            profile_crypto::write_json(path, CONNECTIONS_PURPOSE, &file)?;
        }
        return Ok(file);
    }

    let data = fs::read_to_string(path).map_err(|error| error.to_string())?;
    parse_connections(serde_json::from_str(&data).map_err(|error| error.to_string())?)
}

pub fn load_connections(path: &Path) -> Result<Vec<SavedConnection>, String> {
    Ok(load_connections_file(path)?.connections)
}

pub fn save_connections_file(path: &Path, file: &ConnectionsFile) -> Result<(), String> {
    if profile_crypto::managed_profile_path(path) || profile_crypto::file_is_encrypted(path) {
        let mut protected = file.clone();
        protect_file_secrets(&mut protected)?;
        return profile_crypto::write_json(path, CONNECTIONS_PURPOSE, &protected);
    }

    let data = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    fs::write(path, data).map_err(|error| error.to_string())
}

pub fn save_connections(path: &Path, connections: &[SavedConnection]) -> Result<(), String> {
    let existing = load_connections_file(path).unwrap_or_default();
    let file = ConnectionsFile {
        connections: connections.to_vec(),
        groups: existing.groups,
    };
    save_connections_file(path, &file)
}

pub fn load_groups(path: &Path) -> Result<Vec<ConnectionGroup>, String> {
    Ok(load_connections_file(path)?.groups)
}

pub fn save_groups(path: &Path, groups: &[ConnectionGroup]) -> Result<(), String> {
    let mut file = load_connections_file(path).unwrap_or_default();
    file.groups = groups.to_vec();
    save_connections_file(path, &file)
}

fn protect_ssh_secrets(connection: &mut SshConnection) -> Result<bool, String> {
    let mut changed = connection.save_in_keychain != Some(true);
    if let Some(password) = connection
        .password
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        keychain_utils::set_ssh_password(&connection.id, password)?;
        changed = true;
    }
    if let Some(passphrase) = connection
        .key_passphrase
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        keychain_utils::set_ssh_key_passphrase(&connection.id, passphrase)?;
        changed = true;
    }
    changed |= connection.password.is_some() || connection.key_passphrase.is_some();
    connection.password = None;
    connection.key_passphrase = None;
    connection.save_in_keychain = Some(true);
    Ok(changed)
}

pub fn load_ssh_connections_file(path: &Path) -> Result<Vec<SshConnection>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    if profile_crypto::managed_profile_path(path) || profile_crypto::file_is_encrypted(path) {
        let read = profile_crypto::read_json::<Vec<SshConnection>>(path, SSH_CONNECTIONS_PURPOSE)?
            .ok_or_else(|| "SSH profile disappeared while it was being read".to_string())?;
        let mut connections = read.value;
        let mut recovered = false;
        // One-time conversion of encrypted profiles back to plain JSON, merging
        // entries that only survive in the legacy backup.
        let was_encrypted = !read.was_plaintext;
        if was_encrypted {
            if let Some(backup) = profile_crypto::read_legacy_backup::<Vec<SshConnection>>(
                path,
                SSH_CONNECTIONS_PURPOSE,
            )? {
                let mut ids = connections
                    .iter()
                    .map(|connection| connection.id.clone())
                    .collect::<std::collections::HashSet<_>>();
                for connection in backup {
                    if ids.insert(connection.id.clone()) {
                        connections.push(connection);
                        recovered = true;
                    }
                }
            }
        }
        let changed = connections
            .iter_mut()
            .try_fold(recovered, |changed, connection| {
                protect_ssh_secrets(connection)
                    .map(|connection_changed| changed || connection_changed)
            })?;
        if was_encrypted || changed {
            profile_crypto::write_json(path, SSH_CONNECTIONS_PURPOSE, &connections)?;
        }
        return Ok(connections);
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

pub fn save_ssh_connections_file(path: &Path, connections: &[SshConnection]) -> Result<(), String> {
    if profile_crypto::managed_profile_path(path) || profile_crypto::file_is_encrypted(path) {
        let mut protected = connections.to_vec();
        for connection in &mut protected {
            protect_ssh_secrets(connection)?;
        }
        return profile_crypto::write_json(path, SSH_CONNECTIONS_PURPOSE, &protected);
    }
    let data = serde_json::to_string_pretty(connections).map_err(|error| error.to_string())?;
    fs::write(path, data).map_err(|error| error.to_string())
}

pub fn migrate_local_profiles(config_dir: &Path) -> Result<LocalProfileSummary, String> {
    let database_path = crate::paths::resolve_connections_path(config_dir);
    let database_connections = load_connections_file(&database_path)?.connections.len();
    let ssh_path = config_dir.join("ssh_connections.json");
    let ssh_connections = load_ssh_connections_file(&ssh_path)?.len();
    Ok(LocalProfileSummary {
        database_connections,
        ssh_connections,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::models::ConnectionParams;

    #[test]
    #[ignore = "uses isolated OS-keychain entries"]
    fn legacy_profile_migrates_without_changing_connection_ids() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.managed-test.json");
        let first_id = uuid::Uuid::new_v4().to_string();
        let second_id = uuid::Uuid::new_v4().to_string();
        let first_password = uuid::Uuid::new_v4().to_string();
        let second_password = uuid::Uuid::new_v4().to_string();
        let legacy = ConnectionsFile {
            groups: Vec::new(),
            connections: vec![
                SavedConnection {
                    id: first_id.clone(),
                    name: "MySQL test".to_string(),
                    params: ConnectionParams {
                        driver: "mysql".to_string(),
                        host: Some("test-db.example.invalid".to_string()),
                        password: Some(first_password.clone()),
                        save_in_keychain: Some(false),
                        ..ConnectionParams::default()
                    },
                    group_id: None,
                    sort_order: None,
                    detect_json_in_text_columns: None,
                    appearance: None,
                },
                SavedConnection {
                    id: second_id.clone(),
                    name: "Local Postgres".to_string(),
                    params: ConnectionParams {
                        driver: "postgres".to_string(),
                        host: Some("postgres.example.invalid".to_string()),
                        password: Some(second_password.clone()),
                        save_in_keychain: Some(false),
                        ..ConnectionParams::default()
                    },
                    group_id: None,
                    sort_order: None,
                    detect_json_in_text_columns: None,
                    appearance: None,
                },
            ],
        };
        fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let result: Result<(), String> = (|| {
            let migrated = load_connections_file(&path)?;
            let ids = migrated
                .connections
                .iter()
                .map(|connection| connection.id.as_str())
                .collect::<Vec<_>>();
            if ids != [first_id.as_str(), second_id.as_str()] {
                return Err("Connection IDs changed during migration".to_string());
            }
            if migrated.connections.iter().any(|connection| {
                connection.params.password.is_some()
                    || connection.params.save_in_keychain != Some(true)
            }) {
                return Err("Migrated profile still contains an inline password".to_string());
            }
            if migrated.connections[0].params.audit_profile.as_deref() != Some("mysql-test") {
                return Err("Local audit alias was not preserved".to_string());
            }
            if keychain_utils::get_db_password(&first_id, "")? != first_password
                || keychain_utils::get_db_password(&second_id, "")? != second_password
            {
                return Err("Migrated passwords did not reach the OS keychain".to_string());
            }

            // Simulate an older executable replacing the encrypted profile with
            // the empty shape it believes it loaded. The next current launch
            // must merge the encrypted migration backup instead of losing data.
            fs::write(
                &path,
                serde_json::to_vec_pretty(&ConnectionsFile::default()).unwrap(),
            )
            .map_err(|error| error.to_string())?;
            let recovered = load_connections_file(&path)?;
            if recovered.connections.len() != 2
                || recovered.connections[0].id != first_id
                || recovered.connections[1].id != second_id
            {
                return Err("Encrypted rollback did not recover a legacy overwrite".to_string());
            }

            let backup_directory = directory.path().join("migration-backups");
            let backup_files = fs::read_dir(&backup_directory)
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            if backup_files.len() != 1 {
                return Err("Encrypted migration backup was not created".to_string());
            }
            let mut profile_files = vec![path.clone()];
            profile_files.extend(backup_files.into_iter().map(|entry| entry.path()));
            profile_files.push(
                directory
                    .path()
                    .join(".connections.managed-test.json.previous"),
            );
            for profile_file in profile_files.into_iter().filter(|file| file.exists()) {
                let bytes = fs::read(profile_file).map_err(|error| error.to_string())?;
                for plaintext in [
                    first_password.as_bytes(),
                    second_password.as_bytes(),
                    b"test-db.example.invalid".as_slice(),
                ] {
                    if bytes
                        .windows(plaintext.len())
                        .any(|window| window == plaintext)
                    {
                        return Err("A migrated profile file still contains plaintext".to_string());
                    }
                }
            }
            Ok(())
        })();

        let _ = keychain_utils::delete_db_password(&first_id);
        let _ = keychain_utils::delete_db_password(&second_id);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    #[ignore = "uses an isolated OS-keychain entry"]
    fn legacy_ssh_profile_migrates_to_encrypted_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ssh_connections.managed-test.json");
        let id = uuid::Uuid::new_v4().to_string();
        let password = uuid::Uuid::new_v4().to_string();
        let legacy = vec![SshConnection {
            id: id.clone(),
            name: "Private SSH".to_string(),
            host: "ssh.example.invalid".to_string(),
            port: 22,
            user: "fixture-user".to_string(),
            auth_type: Some("password".to_string()),
            password: Some(password.clone()),
            key_file: None,
            key_passphrase: None,
            allow_passphrase_prompt: Some(false),
            save_in_keychain: Some(false),
        }];
        fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let result: Result<(), String> = (|| {
            let migrated = load_ssh_connections_file(&path)?;
            if migrated.len() != 1
                || migrated[0].id != id
                || migrated[0].password.is_some()
                || migrated[0].save_in_keychain != Some(true)
            {
                return Err(
                    "SSH profile migration changed its identity or retained a secret".to_string(),
                );
            }
            if keychain_utils::get_ssh_password(&id, "")? != password {
                return Err("SSH password did not reach the OS keychain".to_string());
            }
            fs::write(&path, b"[]").map_err(|error| error.to_string())?;
            let recovered = load_ssh_connections_file(&path)?;
            if recovered.len() != 1 || recovered[0].id != id {
                return Err("Encrypted SSH rollback did not recover a legacy overwrite".to_string());
            }
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            for plaintext in [password.as_bytes(), b"ssh.example.invalid".as_slice()] {
                if bytes
                    .windows(plaintext.len())
                    .any(|window| window == plaintext)
                {
                    return Err("SSH profile metadata remained plaintext".to_string());
                }
            }
            Ok(())
        })();

        let _ = keychain_utils::delete_ssh_password(&id);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

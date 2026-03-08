use crate::keychain_utils;
use crate::models::{ConnectionGroup, ConnectionsFile, SavedConnection};
use std::fs;
use std::path::Path;

/// Load connections file, supporting both old format (array of connections) and new format (with groups)
pub fn load_connections_file(path: &Path) -> Result<ConnectionsFile, String> {
    if !path.exists() {
        return Ok(ConnectionsFile::default());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    // Try parsing as the new format first
    if let Ok(file) = serde_json::from_str::<ConnectionsFile>(&content) {
        let mut result = file;
        populate_keychain_passwords(&mut result.connections);
        return Ok(result);
    }

    // Fall back to old format (array of connections)
    let connections: Vec<SavedConnection> = serde_json::from_str(&content)
        .map_err(|_| "Failed to parse connections file".to_string())?;

    let mut result = ConnectionsFile {
        groups: Vec::new(),
        connections,
    };
    populate_keychain_passwords(&mut result.connections);
    Ok(result)
}

/// Legacy function for backward compatibility
pub fn load_connections(path: &Path) -> Result<Vec<SavedConnection>, String> {
    let file = load_connections_file(path)?;
    Ok(file.connections)
}

fn populate_keychain_passwords(connections: &mut [SavedConnection]) {
    for conn in connections {
        if conn.params.save_in_keychain.unwrap_or(false) {
            match keychain_utils::get_db_password(&conn.id) {
                Ok(pwd) => conn.params.password = Some(pwd),
                Err(e) => eprintln!(
                    "[Keyring Error] Failed to get DB password for {}: {}",
                    conn.id, e
                ),
            }
            if conn.params.ssh_enabled.unwrap_or(false) {
                if let Ok(ssh_pwd) = keychain_utils::get_ssh_password(&conn.id) {
                    if !ssh_pwd.trim().is_empty() {
                        conn.params.ssh_password = Some(ssh_pwd);
                    }
                }
            }
        }
    }
}

pub fn save_connections_file(path: &Path, file: &ConnectionsFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    // Create a copy to sanitize passwords before saving to JSON
    let mut connections_to_save = Vec::new();
    for conn in &file.connections {
        let mut c = conn.clone();
        if c.params.save_in_keychain.unwrap_or(false) {
            // Passwords are stored in keychain, remove from JSON
            c.params.password = None;
            c.params.ssh_password = None;
        }
        connections_to_save.push(c);
    }

    let to_save = ConnectionsFile {
        groups: file.groups.clone(),
        connections: connections_to_save,
    };

    let json = serde_json::to_string_pretty(&to_save).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// Legacy function for backward compatibility - saves using new format
pub fn save_connections(path: &Path, connections: &[SavedConnection]) -> Result<(), String> {
    // Load existing groups if any
    let existing = load_connections_file(path).unwrap_or_default();
    let file = ConnectionsFile {
        groups: existing.groups,
        connections: connections.to_vec(),
    };
    save_connections_file(path, &file)
}

pub fn load_groups(path: &Path) -> Result<Vec<ConnectionGroup>, String> {
    let file = load_connections_file(path)?;
    Ok(file.groups)
}

pub fn save_groups(path: &Path, groups: &[ConnectionGroup]) -> Result<(), String> {
    let mut file = load_connections_file(path).unwrap_or_default();
    file.groups = groups.to_vec();
    save_connections_file(path, &file)
}

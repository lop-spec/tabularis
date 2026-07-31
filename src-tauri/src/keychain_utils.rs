use keyring::Entry;

const SERVICE_NAME: &str = "tabularis";

pub fn set_db_password(connection_id: &str, password: &str) -> Result<(), String> {
    eprintln!("[Keychain] Setting DB password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:db", connection_id)).map_err(|e| e.to_string())?;
    entry.set_password(password).map_err(|e| {
        eprintln!("[Keychain] Error setting password: {}", e);
        e.to_string()
    })
}

pub fn get_db_password(connection_id: &str, connection_name: &str) -> Result<String, String> {
    if connection_name.is_empty() {
        eprintln!("[Keychain] Getting DB password for {}", connection_id);
    } else {
        eprintln!(
            "[Keychain] Getting DB password for {} ({})",
            connection_name, connection_id
        );
    }
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:db", connection_id)).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => {
            eprintln!("[Keychain] Password found for {}", connection_id);
            Ok(pwd)
        }
        Err(e) => {
            eprintln!(
                "[Keychain] Error getting password for {}: {}",
                connection_id, e
            );
            Err(e.to_string())
        }
    }
}

pub fn delete_db_password(connection_id: &str) -> Result<(), String> {
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:db", connection_id)).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_connection_uri(connection_id: &str, connection_uri: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, &format!("{}:connection_uri", connection_id))
        .map_err(|e| e.to_string())?;
    entry.set_password(connection_uri).map_err(|e| e.to_string())
}

pub fn get_connection_uri(connection_id: &str) -> Result<String, String> {
    let entry = Entry::new(SERVICE_NAME, &format!("{}:connection_uri", connection_id))
        .map_err(|e| e.to_string())?;
    entry.get_password().map_err(|e| e.to_string())
}

pub fn delete_connection_uri(connection_id: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, &format!("{}:connection_uri", connection_id))
        .map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_ssh_password(connection_id: &str, password: &str) -> Result<(), String> {
    eprintln!("[Keychain] Setting SSH password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:ssh", connection_id)).map_err(|e| e.to_string())?;
    entry.set_password(password).map_err(|e| {
        eprintln!("[Keychain] Error setting SSH password: {}", e);
        e.to_string()
    })
}

pub fn get_ssh_password(connection_id: &str, connection_name: &str) -> Result<String, String> {
    if connection_name.is_empty() {
        eprintln!("[Keychain] Getting SSH password for {}", connection_id);
    } else {
        eprintln!(
            "[Keychain] Getting SSH password for {} ({})",
            connection_name, connection_id
        );
    }
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:ssh", connection_id)).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => {
            eprintln!("[Keychain] SSH Password found for {}", connection_id);
            Ok(pwd)
        }
        Err(e) => {
            eprintln!(
                "[Keychain] Error getting SSH password for {}: {}",
                connection_id, e
            );
            Err(e.to_string())
        }
    }
}

pub fn delete_ssh_password(connection_id: &str) -> Result<(), String> {
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:ssh", connection_id)).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_ssh_key_passphrase(connection_id: &str, passphrase: &str) -> Result<(), String> {
    eprintln!(
        "[Keychain] Setting SSH key passphrase for {}",
        connection_id
    );
    let entry = Entry::new(SERVICE_NAME, &format!("{}:ssh_passphrase", connection_id))
        .map_err(|e| e.to_string())?;
    entry.set_password(passphrase).map_err(|e| {
        eprintln!("[Keychain] Error setting SSH key passphrase: {}", e);
        e.to_string()
    })
}

pub fn get_ssh_key_passphrase(
    connection_id: &str,
    connection_name: &str,
) -> Result<String, String> {
    if connection_name.is_empty() {
        eprintln!(
            "[Keychain] Getting SSH key passphrase for {}",
            connection_id
        );
    } else {
        eprintln!(
            "[Keychain] Getting SSH key passphrase for {} ({})",
            connection_name, connection_id
        );
    }
    let entry = Entry::new(SERVICE_NAME, &format!("{}:ssh_passphrase", connection_id))
        .map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => {
            eprintln!("[Keychain] SSH key passphrase found for {}", connection_id);
            Ok(pwd)
        }
        Err(e) => {
            eprintln!(
                "[Keychain] Error getting SSH key passphrase for {}: {}",
                connection_id, e
            );
            Err(e.to_string())
        }
    }
}

pub fn delete_ssh_key_passphrase(connection_id: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, &format!("{}:ssh_passphrase", connection_id))
        .map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

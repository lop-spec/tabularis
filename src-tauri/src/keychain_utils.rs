use keyring::Entry;

const SERVICE_NAME: &str = "tabularis";

pub fn set_db_password(connection_id: &str, password: &str) -> Result<(), String> {
    println!("[Keychain] Setting DB password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:db", connection_id)).map_err(|e| e.to_string())?;
    entry.set_password(password).map_err(|e| {
        println!("[Keychain] Error setting password: {}", e);
        e.to_string()
    })
}

pub fn get_db_password(connection_id: &str) -> Result<String, String> {
    println!("[Keychain] Getting DB password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:db", connection_id)).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => {
            println!("[Keychain] Password found for {}", connection_id);
            Ok(pwd)
        }
        Err(e) => {
            println!(
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

pub fn set_ssh_password(connection_id: &str, password: &str) -> Result<(), String> {
    println!("[Keychain] Setting SSH password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:ssh", connection_id)).map_err(|e| e.to_string())?;
    entry.set_password(password).map_err(|e| {
        println!("[Keychain] Error setting SSH password: {}", e);
        e.to_string()
    })
}

pub fn get_ssh_password(connection_id: &str) -> Result<String, String> {
    println!("[Keychain] Getting SSH password for {}", connection_id);
    let entry =
        Entry::new(SERVICE_NAME, &format!("{}:ssh", connection_id)).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => {
            println!("[Keychain] SSH Password found for {}", connection_id);
            Ok(pwd)
        }
        Err(e) => {
            println!(
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

pub fn set_ai_key(provider: &str, key: &str) -> Result<(), String> {
    println!("[Keychain] Setting AI key for {}", provider);
    let entry =
        Entry::new(SERVICE_NAME, &format!("ai_key:{}", provider)).map_err(|e| e.to_string())?;
    entry.set_password(key).map_err(|e| {
        println!("[Keychain] Error setting AI key: {}", e);
        e.to_string()
    })
}

pub fn get_ai_key(provider: &str) -> Result<String, String> {
    println!("[Keychain] Getting AI key for {}", provider);
    let entry =
        Entry::new(SERVICE_NAME, &format!("ai_key:{}", provider)).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pwd) => Ok(pwd),
        Err(keyring::Error::NoEntry) => Err("No key found".to_string()),
        Err(e) => {
            println!("[Keychain] Error getting AI key for {}: {}", provider, e);
            Err(e.to_string())
        }
    }
}

pub fn delete_ai_key(provider: &str) -> Result<(), String> {
    let entry =
        Entry::new(SERVICE_NAME, &format!("ai_key:{}", provider)).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub static ENGINE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gh-ost.bin"));
static LICENSE: &str = include_str!(concat!(env!("OUT_DIR"), "/gh-ost.LICENSE"));

pub fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir(path)
        .map_err(|e| format!("Could not create private migration directory: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let user = std::env::var("USERNAME").map_err(|_| "Current Windows user is unknown")?;
        let domain =
            std::env::var("USERDOMAIN").map_err(|_| "Current Windows domain is unknown")?;
        let root = std::env::var("SystemRoot").map_err(|_| "SystemRoot is missing")?;
        let output = std::process::Command::new(Path::new(&root).join("System32/icacls.exe"))
            .arg(path)
            .args([
                "/inheritance:r",
                "/grant:r",
                &format!("{domain}\\{user}:(OI)(CI)F"),
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(
                "Could not restrict the migration directory ACL; no credentials were written"
                    .into(),
            );
        }
    }
    Ok(())
}

pub fn engine_path(root: &Path) -> Result<PathBuf, String> {
    if ENGINE.is_empty() {
        return Err("This build does not contain gh-ost. Install a CI-built Tabularis release with Online DDL support.".into());
    }
    let digest = format!("{:x}", Sha256::digest(ENGINE));
    let directory = root.join("engines").join(&digest);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let target = directory.join(if cfg!(windows) {
        "gh-ost.exe"
    } else {
        "gh-ost"
    });
    if target.exists() {
        let existing = fs::read(&target).map_err(|e| e.to_string())?;
        if Sha256::digest(&existing) != Sha256::digest(ENGINE) {
            return Err(
                "Cached gh-ost checksum mismatch; refusing to execute or overwrite it".into(),
            );
        }
    } else {
        let temp = directory.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&temp, ENGINE).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        fs::rename(&temp, &target).map_err(|e| e.to_string())?;
    }
    fs::write(directory.join("LICENSE.gh-ost"), LICENSE).map_err(|e| e.to_string())?;
    Ok(target)
}

pub fn remove_credentials(directory: &Path) {
    if let Err(error) = fs::remove_file(directory.join("credentials.cnf")) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::error!("Could not remove gh-ost credential file: {error}");
        }
    }
}

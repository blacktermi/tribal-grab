//! Mot de passe du proxy dans le trousseau macOS (commande `security`), jamais dans SQLite.
use crate::error::{Error, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const SERVICE: &str = "Tribal Grab";
const ACCOUNT: &str = "proxy";

pub fn set_proxy_password(pw: &str) -> Result<()> {
    // `security -i` lit la commande sur l'entrée standard : le mot de passe n'apparaît pas dans `ps`.
    let esc = pw.replace('\\', "\\\\").replace('"', "\\\"");
    let mut child = Command::new("/usr/bin/security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Other(format!("trousseau indisponible : {e}")))?;
    let line = format!("add-generic-password -U -s \"{SERVICE}\" -a \"{ACCOUNT}\" -w \"{esc}\"\n");
    child.stdin.take().unwrap().write_all(line.as_bytes())?;
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Other("impossible d'écrire dans le trousseau".into()))
    }
}

pub fn get_proxy_password() -> Option<String> {
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string())
}

pub fn delete_proxy_password() {
    let _ = Command::new("/usr/bin/security").args(["delete-generic-password", "-s", SERVICE, "-a", ACCOUNT]).output();
}

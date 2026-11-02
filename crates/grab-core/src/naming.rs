//! Modèle de nom de fichier : variables, nettoyage, longueur, collisions.
use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_COMPONENT_BYTES: usize = 180;

pub const VARIABLES: [&str; 14] = [
    "title", "id", "site", "host", "uploader", "upload_date", "timestamp", "playlist", "playlist_index",
    "playlist_count", "resolution", "ext", "original_name", "last_path",
];

pub fn validate_template(t: &str) -> Result<()> {
    if !t.contains("{title}") {
        return Err(Error::Other("le modèle de nom doit contenir {title}".into()));
    }
    Ok(())
}

/// Nettoie un seul élément de chemin : jamais de `/`, de `..` ni de caractère de contrôle.
pub fn sanitize_component(s: &str) -> String {
    let c = clean(s);
    if c.is_empty() { "video".to_string() } else { c }
}

/// Comme `sanitize_component`, mais peut retourner une chaîne vide.
fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '/' | '\\' | ':' => out.push('-'),
            '*' | '?' | '"' | '<' | '>' | '|' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == '.' || c == ' ' || c == '-').to_string();
    truncate_bytes(&trimmed, MAX_COMPONENT_BYTES)
}

fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].trim_end().to_string()
}

/// Produit le chemin relatif (sans extension) à partir du modèle. Les valeurs des variables
/// sont nettoyées ; seuls les `/` écrits par l'utilisateur dans le modèle créent des sous-dossiers.
pub fn render(template: &str, vars: &BTreeMap<String, String>) -> PathBuf {
    let template = if template.contains("{title}") { template } else { "{title}" };
    let mut rel = PathBuf::new();
    for part in template.split('/') {
        let mut s = String::new();
        let mut rest = part;
        while let Some(i) = rest.find('{') {
            s.push_str(&rest[..i]);
            match rest[i..].find('}') {
                Some(j) => {
                    let name = &rest[i + 1..i + j];
                    s.push_str(&clean(vars.get(name).map(String::as_str).unwrap_or("")));
                    rest = &rest[i + j + 1..];
                }
                None => {
                    s.push_str(&rest[i..]);
                    rest = "";
                }
            }
        }
        s.push_str(rest);
        let comp = sanitize_component(&s);
        if comp != "." && comp != ".." {
            rel.push(comp);
        }
    }
    rel
}

/// Chemin final libre : ajoute « (1) », « (2) »… tant que le fichier ou son dossier partiel existe.
pub fn unique_path(dir: &Path, rel: &Path, ext: &str) -> PathBuf {
    let base = dir.join(rel);
    let name = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "video".into());
    let parent = base.parent().unwrap_or(dir).to_path_buf();
    let mk = |n: u32| {
        let stem = if n == 0 { name.clone() } else { format!("{name} ({n})") };
        parent.join(if ext.is_empty() { stem } else { format!("{stem}.{ext}") })
    };
    (0..10_000)
        .map(mk)
        .find(|p| !p.exists() && !part_dir(p).exists())
        .unwrap_or_else(|| mk(10_000))
}

pub fn part_dir(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_os_string();
    s.push(".tgpart");
    PathBuf::from(s)
}

/// Garantit que `path` reste sous `dir` (défense en profondeur).
pub fn ensure_inside(dir: &Path, path: &Path) -> Result<()> {
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) || !path.starts_with(dir) {
        return Err(Error::Other("le nom de fichier sort du dossier de destination".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(title: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("title".into(), title.into()), ("site".into(), "YouTube".into()), ("id".into(), "abc".into())])
    }

    #[test]
    fn path_traversal_is_neutralised() {
        for evil in ["../../etc/passwd", "..", "a/../../b", "/etc/passwd", "..\\..\\x", "a\0b"] {
            let rel = render("{title}", &vars(evil));
            assert_eq!(rel.components().count(), 1, "{evil} -> {rel:?}");
            let comp = rel.file_name().unwrap().to_string_lossy().into_owned();
            assert!(comp != ".." && comp != "." && !comp.contains('/') && !comp.contains('\\'), "{rel:?}");
            let p = Path::new("/dl").join(&rel);
            ensure_inside(Path::new("/dl"), &p).unwrap();
        }
    }

    #[test]
    fn template_variables_and_dirs() {
        let rel = render("{site}/{title} [{id}]", &vars("Mon: film?"));
        assert_eq!(rel, PathBuf::from("YouTube/Mon- film [abc]"));
        assert_eq!(render("{playlist}{title}", &vars("x")), PathBuf::from("x"));
    }

    #[test]
    fn long_names_are_cut_on_char_boundary() {
        let long = "é".repeat(500);
        let rel = render("{title}", &vars(&long));
        assert!(rel.to_string_lossy().len() <= MAX_COMPONENT_BYTES);
    }

    #[test]
    fn collisions() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.mp4"), b"x").unwrap();
        let p = unique_path(d.path(), Path::new("a"), "mp4");
        assert_eq!(p.file_name().unwrap(), "a (1).mp4");
    }

    #[test]
    fn template_requires_title() {
        assert!(validate_template("{id}").is_err());
        assert!(validate_template("{title}-{id}").is_ok());
    }
}

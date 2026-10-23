//! Private, account-scoped original images with exact-reference and stable
//! attachment indexes. A stable identity must come from the source message,
//! never a guessed URL with its signature or version parameters removed.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const MAX_IMAGE: u64 = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_CACHE: u64 = 512 * 1024 * 1024;
const MAX_OBJECTS: usize = 2048;

#[derive(Serialize, Deserialize)]
struct Reference {
    version: u8,
    content: String,
    bytes: u64,
}

pub(super) fn root() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| crate::resources::user_home().map(|h| PathBuf::from(h).join(".cache")))
        .map(|p| p.join("stepx-companion/amoo-previews-v2"))
}

fn hash(value: &[u8]) -> String {
    stepx_companion::amoo_media::artifact_content_key(value)
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn directory(root: &Path, scope: &str) -> Option<PathBuf> {
    valid_digest(scope).then(|| root.join(scope))
}
fn reference_keys(url: &str, identity: Option<&str>) -> Vec<String> {
    let mut keys = vec![];
    if !url.is_empty() && url.len() <= 32 * 1024 {
        keys.push(hash(format!("url:{url}").as_bytes()));
    }
    if let Some(identity) = identity.filter(|s| !s.is_empty() && s.len() <= 4096) {
        keys.push(hash(format!("attachment:{identity}").as_bytes()));
    }
    keys
}
fn valid_image(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_IMAGE {
        return false;
    }
    let Ok(reader) = image::ImageReader::open(path) else {
        return false;
    };
    let Ok(reader) = reader.with_guessed_format() else {
        return false;
    };
    let Ok((width, height)) = reader.into_dimensions() else {
        return false;
    };
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return false;
    }
    image::ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.decode().ok())
        .is_some()
}
fn private_directory(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let meta = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "图片缓存目录无效"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn write_reference(directory: &Path, key: &str, reference: &Reference) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut temporary, reference)?;
    temporary.flush()?;
    temporary.persist(directory.join(format!("{key}.json")))?;
    Ok(())
}
fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ =
            file.set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::now()));
    }
}

pub(super) fn load(root: &Path, scope: &str, url: &str) -> Option<PathBuf> {
    load_with_identity(root, scope, url, None)
}

/// Reads cached originals before consulting the network. Identity is scoped to
/// an immutable source message + attachment slot, supplied by the caller.
pub(super) fn load_with_identity(
    root: &Path,
    scope: &str,
    url: &str,
    identity: Option<&str>,
) -> Option<PathBuf> {
    let directory = directory(root, scope)?;
    let refs = directory.join("refs");
    let keys = reference_keys(url, identity);
    for key in &keys {
        let reference_path = refs.join(format!("{key}.json"));
        let meta = std::fs::symlink_metadata(&reference_path).ok();
        if !meta.is_some_and(|m| m.is_file() && m.len() <= 1024) {
            continue;
        }
        let Some(reference) = std::fs::read(&reference_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Reference>(&bytes).ok())
        else {
            continue;
        };
        if reference.version != 2 || !valid_digest(&reference.content) {
            continue;
        }
        let path = directory
            .join("objects")
            .join(format!("{}.image", reference.content));
        if !valid_image(&path) {
            continue;
        }
        let Some(bytes) = std::fs::read(&path).ok() else {
            continue;
        };
        if bytes.len() as u64 != reference.bytes || hash(&bytes) != reference.content {
            continue;
        }
        // Teach an exact newly signed URL about an already verified source
        // attachment. Nothing is stripped from the URL or sent to the network.
        for alias in &keys {
            let _ = write_reference(&refs, alias, &reference);
        }
        touch(&path);
        return Some(path);
    }
    // Migrate only an exact known reference from the old cache. Never scan by
    // file name or share cached originals between phone accounts.
    if !url.is_empty() {
        let filename = format!(
            "{}.image",
            stepx_companion::amoo_media::artifact_cache_key(url)
        );
        let mut legacy_paths = vec![directory.join(&filename)];
        if root
            .file_name()
            .is_some_and(|name| name == "amoo-previews-v2")
        {
            if let Some(parent) = root.parent() {
                legacy_paths.push(parent.join("amoo-previews-v1").join(scope).join(&filename));
            }
        }
        for legacy in legacy_paths {
            if valid_image(&legacy) {
                if let Ok(path) = save_with_identity(root, scope, url, &legacy, identity) {
                    let _ = std::fs::remove_file(legacy);
                    return Some(path);
                }
            }
        }
    }
    None
}

pub(super) fn save(root: &Path, scope: &str, url: &str, source: &Path) -> Result<PathBuf> {
    save_with_identity(root, scope, url, source, None)
}

/// Stores original bytes once per account, with hashed exact-reference and
/// stable message/attachment aliases. The limit is 512 MiB / 2048 originals per
/// account; least recently viewed originals are evicted, explicit exports are not.
pub(super) fn save_with_identity(
    root: &Path,
    scope: &str,
    url: &str,
    source: &Path,
    identity: Option<&str>,
) -> Result<PathBuf> {
    anyhow::ensure!(valid_image(source), "图片预览内容无效");
    let directory = directory(root, scope).context("尚未核实图片所属账号")?;
    let keys = reference_keys(url, identity);
    anyhow::ensure!(!keys.is_empty(), "图片缓存缺少有效来源");
    private_directory(&directory)?;
    let objects = directory.join("objects");
    let refs = directory.join("refs");
    private_directory(&objects)?;
    private_directory(&refs)?;
    let bytes = std::fs::read(source)?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_IMAGE, "图片超过缓存限制");
    let content = hash(&bytes);
    let path = objects.join(format!("{content}.image"));
    if !valid_image(&path)
        || std::fs::read(&path)
            .ok()
            .is_none_or(|stored| hash(&stored) != content)
    {
        let mut temporary = tempfile::NamedTempFile::new_in(&objects)?;
        temporary.write_all(&bytes)?;
        temporary.flush()?;
        temporary.persist(&path)?;
    }
    let reference = Reference {
        version: 2,
        content,
        bytes: bytes.len() as u64,
    };
    for key in keys {
        write_reference(&refs, &key, &reference)?;
    }
    touch(&path);
    prune(&directory, &path, MAX_CACHE, MAX_OBJECTS)?;
    Ok(path)
}

fn prune(directory: &Path, keep: &Path, max_bytes: u64, max_objects: usize) -> Result<()> {
    let mut files = std::fs::read_dir(directory.join("objects"))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).ok()?;
            (path.extension().is_some_and(|s| s == "image") && meta.is_file())
                .then(|| (path, meta.len(), meta.modified().ok()))
        })
        .collect::<Vec<_>>();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    let mut count = files.len();
    files.sort_by_key(|(_, _, modified)| *modified);
    for (old, size, _) in files {
        if total <= max_bytes && count <= max_objects {
            break;
        }
        if old != keep && std::fs::remove_file(old).is_ok() {
            total = total.saturating_sub(size);
            count = count.saturating_sub(1);
        }
    }
    // References are tiny, but prune stale aliases as originals are evicted.
    for entry in std::fs::read_dir(directory.join("refs"))?.flatten() {
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.len() > 1024 {
            continue;
        }
        let valid = std::fs::read(&path)
            .ok()
            .and_then(|v| serde_json::from_slice::<Reference>(&v).ok())
            .is_some_and(|r| {
                valid_digest(&r.content)
                    && directory
                        .join("objects")
                        .join(format!("{}.image", r.content))
                        .is_file()
            });
        if !valid {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(root: &Path, name: &str, red: u8) -> PathBuf {
        let path = root.join(name);
        image::RgbaImage::from_pixel(2, 2, image::Rgba([red, 20, 30, 255]))
            .save(&path)
            .unwrap();
        path
    }
    #[test]
    fn cached_original_survives_source_deletion_restart_and_expired_url() {
        let root = tempfile::tempdir().unwrap();
        let image = source(root.path(), "source.png", 10);
        let scope = "a".repeat(64);
        let original = std::fs::read(&image).unwrap();
        let saved = save_with_identity(
            root.path(),
            &scope,
            "https://example.test/image?signature=old",
            &image,
            Some("session/message/0"),
        )
        .unwrap();
        std::fs::remove_file(image).unwrap();
        assert_eq!(
            std::fs::read(
                load_with_identity(
                    root.path(),
                    &scope,
                    "https://example.test/image?signature=new",
                    Some("session/message/0")
                )
                .unwrap()
            )
            .unwrap(),
            original
        );
        // Newly signed exact reference is now linked too; no network is involved.
        assert_eq!(
            load(
                root.path(),
                &scope,
                "https://example.test/image?signature=new"
            ),
            Some(saved)
        );
    }
    #[test]
    fn accounts_and_distinct_attachments_do_not_alias() {
        let root = tempfile::tempdir().unwrap();
        let image = source(root.path(), "source.png", 10);
        let scope = "a".repeat(64);
        let url = "https://example.test/image?version=1&signature=old";
        save_with_identity(root.path(), &scope, url, &image, Some("session/message/0")).unwrap();
        assert!(
            load_with_identity(root.path(), &"b".repeat(64), url, Some("session/message/0"))
                .is_none()
        );
        assert!(load_with_identity(
            root.path(),
            &scope,
            "https://example.test/image?version=2",
            Some("session/other-message/0")
        )
        .is_none());
        assert!(load(
            root.path(),
            &scope,
            "https://example.test/image?version=1&signature=new"
        )
        .is_none());
        assert!(load(root.path(), "../account", url).is_none());
    }
    #[test]
    fn old_exact_url_cache_migrates_and_checks_corruption() {
        let parent = tempfile::tempdir().unwrap();
        let old = parent.path().join("amoo-previews-v1").join("a".repeat(64));
        std::fs::create_dir_all(&old).unwrap();
        let url = "https://example.test/result.png?signature=old";
        let legacy = old.join(format!(
            "{}.image",
            stepx_companion::amoo_media::artifact_cache_key(url)
        ));
        let image = source(parent.path(), "source.png", 10);
        std::fs::copy(&image, &legacy).unwrap();
        let root = parent.path().join("amoo-previews-v2");
        let migrated = load_with_identity(&root, &"a".repeat(64), url, Some("message/0")).unwrap();
        assert!(!legacy.exists());
        // A different valid image is still rejected if its content hash changed.
        let other = source(parent.path(), "other.png", 99);
        std::fs::copy(other, migrated).unwrap();
        assert!(load(&root, &"a".repeat(64), url).is_none());
    }
    #[test]
    fn identical_original_is_stored_once_and_oldest_is_pruned() {
        let root = tempfile::tempdir().unwrap();
        let image = source(root.path(), "source.png", 10);
        let scope = "a".repeat(64);
        let first = save(root.path(), &scope, "https://example.test/one", &image).unwrap();
        let same = save(root.path(), &scope, "https://example.test/two", &image).unwrap();
        assert_eq!(first, same);
        let second_image = source(root.path(), "second.png", 99);
        let second = save(
            root.path(),
            &scope,
            "https://example.test/three",
            &second_image,
        )
        .unwrap();
        prune(&root.path().join(&scope), &second, MAX_IMAGE, 1).unwrap();
        assert!(!first.exists());
        assert!(load(root.path(), &scope, "https://example.test/one").is_none());
        assert_eq!(
            load(root.path(), &scope, "https://example.test/three"),
            Some(second)
        );
    }
    #[cfg(unix)]
    #[test]
    fn originals_are_private_and_symlink_sources_are_rejected() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let image = source(root.path(), "source.png", 10);
        let link = root.path().join("link.png");
        symlink(&image, &link).unwrap();
        assert!(save(
            root.path(),
            &"a".repeat(64),
            "https://example.test/one",
            &link
        )
        .is_err());
        let saved = save(
            root.path(),
            &"a".repeat(64),
            "https://example.test/one",
            &image,
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(saved).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

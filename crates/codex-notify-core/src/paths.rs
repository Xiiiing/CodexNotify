use crate::error::{CoreError, CoreResult};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

const STORAGE_SCHEMA_VERSION: u32 = 1;
const STORAGE_FILE: &str = "storage.json";
const PORTABLE_MARKER: &str = ".codex-notify-portable";
const PORTABLE_DATA_DIR: &str = "CodexNotifyData";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StorageMode {
    Default,
    Portable,
    Custom,
    Environment,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub configured: bool,
    pub mode: StorageMode,
    pub root: String,
    pub config_dir: String,
    pub data_dir: String,
    pub log_dir: String,
    pub locator_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageLocator {
    schema_version: u32,
    mode: StorageMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root: Option<PathBuf>,
}

impl AppPaths {
    pub fn discover() -> CoreResult<Self> {
        Ok(Self::resolve_storage()?.0)
    }

    pub fn resolve_storage() -> CoreResult<(Self, StorageInfo)> {
        let default = Self::system_default()?;
        let locator_file = default.config_dir.join(STORAGE_FILE);

        if let Some(root) = std::env::var_os("CODEX_NOTIFY_DATA_DIR") {
            let root = PathBuf::from(root);
            let paths = Self::from_root(&root);
            return Ok((
                paths.clone(),
                storage_info(true, StorageMode::Environment, &root, &paths, &locator_file),
            ));
        }

        if let Ok(base) = launcher_directory() {
            if base.join(PORTABLE_MARKER).exists() {
                let root = base.join(PORTABLE_DATA_DIR);
                let paths = Self::from_root(&root);
                return Ok((
                    paths.clone(),
                    storage_info(true, StorageMode::Portable, &root, &paths, &locator_file),
                ));
            }
        }

        if locator_file.exists() {
            let locator: StorageLocator = serde_json::from_slice(&std::fs::read(&locator_file)?)?;
            if locator.schema_version != STORAGE_SCHEMA_VERSION {
                return Err(CoreError::InvalidConfig(
                    "unsupported storage configuration schema".into(),
                ));
            }
            let (paths, root) = match locator.mode {
                StorageMode::Default => (default.clone(), default_root(&default)),
                StorageMode::Portable | StorageMode::Custom => {
                    let root = locator.root.ok_or_else(|| {
                        CoreError::InvalidConfig("storage location is missing its root path".into())
                    })?;
                    (Self::from_root(&root), root)
                }
                StorageMode::Environment => {
                    return Err(CoreError::InvalidConfig(
                        "environment storage mode cannot be persisted".into(),
                    ))
                }
            };
            return Ok((
                paths.clone(),
                storage_info(true, locator.mode, &root, &paths, &locator_file),
            ));
        }

        let configured = default.settings_file().exists();
        let root = default_root(&default);
        Ok((
            default.clone(),
            storage_info(
                configured,
                StorageMode::Default,
                &root,
                &default,
                &locator_file,
            ),
        ))
    }

    pub fn configure_storage(
        mode: StorageMode,
        custom_root: Option<&Path>,
    ) -> CoreResult<StorageInfo> {
        if mode == StorageMode::Environment {
            return Err(CoreError::InvalidConfig(
                "environment storage is selected with CODEX_NOTIFY_DATA_DIR".into(),
            ));
        }
        let default = Self::system_default()?;
        let locator_file = default.config_dir.join(STORAGE_FILE);
        let (paths, root) = storage_target(mode, custom_root, &default)?;

        paths.ensure()?;
        verify_writable(&paths.config_dir)?;
        verify_writable(&paths.data_dir)?;
        verify_writable(&paths.log_dir)?;

        persist_storage(mode, &root, &default, &locator_file)?;
        Ok(storage_info(true, mode, &root, &paths, &locator_file))
    }

    /// Copies every non-secret application file to a new location, switches the shared locator,
    /// then removes the previous application directories. Credential-store entries are external
    /// to these paths and intentionally remain untouched.
    pub fn migrate_storage(
        mode: StorageMode,
        custom_root: Option<&Path>,
    ) -> CoreResult<StorageInfo> {
        if mode == StorageMode::Environment || std::env::var_os("CODEX_NOTIFY_DATA_DIR").is_some() {
            return Err(CoreError::InvalidConfig(
                "storage cannot be migrated while CODEX_NOTIFY_DATA_DIR is active".into(),
            ));
        }
        let (current, current_info) = Self::resolve_storage()?;
        if !current_info.configured {
            return Self::configure_storage(mode, custom_root);
        }
        let default = Self::system_default()?;
        let locator_file = default.config_dir.join(STORAGE_FILE);
        let (target, root) = storage_target(mode, custom_root, &default)?;
        validate_migration_paths(&current, &target)?;
        ensure_target_empty(&target, &locator_file)?;
        target.ensure()?;
        verify_writable(&target.config_dir)?;
        verify_writable(&target.data_dir)?;
        verify_writable(&target.log_dir)?;

        let prepared = copy_application_data(&current, &target, &locator_file)
            .and_then(|_| validate_migrated_data(&current, &target));
        if let Err(error) = prepared {
            let _ = cleanup_target(&target, &default, &locator_file);
            return Err(error);
        }
        if let Err(error) = persist_storage(mode, &root, &default, &locator_file) {
            let _ = cleanup_target(&target, &default, &locator_file);
            return Err(error);
        }
        cleanup_previous(&current, &target, &default, &locator_file)?;
        Ok(storage_info(true, mode, &root, &target, &locator_file))
    }

    pub fn system_default() -> CoreResult<Self> {
        let dirs = ProjectDirs::from("com", "Xiiiing", "CodexNotify").ok_or_else(|| {
            CoreError::InvalidConfig("platform application directories are unavailable".into())
        })?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            log_dir: dirs.data_local_dir().join("logs"),
        })
    }

    pub fn ensure(&self) -> CoreResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn events_db(&self) -> PathBuf {
        self.data_dir.join("events.sqlite3")
    }
    pub fn health_file(&self) -> PathBuf {
        self.data_dir.join("hook-health.json")
    }

    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            log_dir: root.join("logs"),
        }
    }
}

fn storage_target(
    mode: StorageMode,
    custom_root: Option<&Path>,
    default: &AppPaths,
) -> CoreResult<(AppPaths, PathBuf)> {
    match mode {
        StorageMode::Default => Ok((default.clone(), default_root(default))),
        StorageMode::Portable => {
            let root = launcher_directory()?.join(PORTABLE_DATA_DIR);
            Ok((AppPaths::from_root(&root), root))
        }
        StorageMode::Custom => {
            let requested = custom_root
                .filter(|path| path.is_absolute())
                .ok_or_else(|| {
                    CoreError::InvalidConfig(
                        "custom storage location must be an absolute path".into(),
                    )
                })?;
            std::fs::create_dir_all(requested)?;
            let root = std::fs::canonicalize(requested)?;
            Ok((AppPaths::from_root(&root), root))
        }
        StorageMode::Environment => Err(CoreError::InvalidConfig(
            "environment storage is selected with CODEX_NOTIFY_DATA_DIR".into(),
        )),
    }
}

fn persist_storage(
    mode: StorageMode,
    root: &Path,
    default: &AppPaths,
    locator_file: &Path,
) -> CoreResult<()> {
    // Removing an old portable marker first is safe because the existing locator still resolves
    // the old portable root. The locator is then committed atomically before a new marker is
    // created, so a failed locator write never selects an unverified destination.
    if let Ok(portable_base) = launcher_directory() {
        let marker = portable_base.join(PORTABLE_MARKER);
        if mode != StorageMode::Portable && marker.exists() {
            std::fs::remove_file(&marker)?;
        }
    }
    std::fs::create_dir_all(&default.config_dir)?;
    atomic_json(
        locator_file,
        &StorageLocator {
            schema_version: STORAGE_SCHEMA_VERSION,
            mode,
            root: (mode != StorageMode::Default).then_some(root.to_path_buf()),
        },
    )?;
    if mode == StorageMode::Portable {
        if let Ok(portable_base) = launcher_directory() {
            if let Err(error) = std::fs::write(
                portable_base.join(PORTABLE_MARKER),
                b"CodexNotify portable storage v1\n",
            ) {
                tracing::warn!(%error, "portable marker could not be written; locator remains active");
            }
        }
    }
    Ok(())
}

fn validate_migration_paths(current: &AppPaths, target: &AppPaths) -> CoreResult<()> {
    let current_paths = [&current.config_dir, &current.data_dir, &current.log_dir];
    let target_paths = [&target.config_dir, &target.data_dir, &target.log_dir];
    if current_paths == target_paths {
        return Err(CoreError::InvalidConfig(
            "the selected storage location is already in use".into(),
        ));
    }
    for source in current_paths {
        for destination in target_paths {
            if source.starts_with(destination) || destination.starts_with(source) {
                return Err(CoreError::InvalidConfig(
                    "the new storage location cannot overlap the current location".into(),
                ));
            }
        }
    }
    Ok(())
}

fn ensure_target_empty(target: &AppPaths, locator_file: &Path) -> CoreResult<()> {
    let mut seen = HashSet::new();
    for directory in [&target.config_dir, &target.data_dir, &target.log_dir] {
        if !directory.exists() || !seen.insert(directory.clone()) {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path == locator_file || path.is_dir() && directory_contains_no_files(&path)? {
                continue;
            }
            return Err(CoreError::InvalidConfig(
                "the selected storage location already contains application data".into(),
            ));
        }
    }
    Ok(())
}

fn directory_contains_no_files(directory: &Path) -> CoreResult<bool> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if !kind.is_dir() || !directory_contains_no_files(&entry.path())? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn copy_application_data(source: &AppPaths, target: &AppPaths, locator: &Path) -> CoreResult<()> {
    copy_tree(
        &source.config_dir,
        &target.config_dir,
        &[locator.to_path_buf()],
    )?;
    copy_tree(
        &source.data_dir,
        &target.data_dir,
        &[
            source.events_db(),
            source.data_dir.join("events.sqlite3-wal"),
            source.data_dir.join("events.sqlite3-shm"),
            source.log_dir.clone(),
        ],
    )?;
    copy_tree(&source.log_dir, &target.log_dir, &[])?;
    backup_database(&source.events_db(), &target.events_db())
}

fn copy_tree(source: &Path, target: &Path, excluded: &[PathBuf]) -> CoreResult<()> {
    if !source.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        if excluded.contains(&source_path) {
            continue;
        }
        let target_path = target.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(CoreError::InvalidConfig(
                "symbolic links are not supported in application storage".into(),
            ));
        }
        if kind.is_dir() {
            copy_tree(&source_path, &target_path, excluded)?;
        } else if kind.is_file() {
            std::fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

fn backup_database(source: &Path, target: &Path) -> CoreResult<()> {
    if !source.exists() {
        return Ok(());
    }
    let source_connection = rusqlite::Connection::open(source)?;
    let mut target_connection = rusqlite::Connection::open(target)?;
    let backup = rusqlite::backup::Backup::new(&source_connection, &mut target_connection)?;
    backup.run_to_completion(32, std::time::Duration::from_millis(20), None)?;
    Ok(())
}

fn validate_migrated_data(source: &AppPaths, target: &AppPaths) -> CoreResult<()> {
    if source.settings_file().exists() && !target.settings_file().is_file() {
        return Err(CoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "settings were not copied",
        )));
    }
    if source.events_db().exists() {
        let connection = rusqlite::Connection::open(target.events_db())?;
        let integrity: String =
            connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(CoreError::Database(rusqlite::Error::InvalidQuery));
        }
    }
    Ok(())
}

fn cleanup_previous(
    previous: &AppPaths,
    target: &AppPaths,
    default: &AppPaths,
    locator_file: &Path,
) -> CoreResult<()> {
    let mut directories = vec![
        previous.config_dir.clone(),
        previous.data_dir.clone(),
        previous.log_dir.clone(),
    ];
    directories.sort_by_key(|path| path.components().count());
    directories.dedup();
    let mut removed_ancestor: Vec<PathBuf> = Vec::new();
    for directory in directories {
        if removed_ancestor
            .iter()
            .any(|parent| directory.starts_with(parent))
            || [&target.config_dir, &target.data_dir, &target.log_dir].contains(&&directory)
        {
            continue;
        }
        if directory == default.config_dir {
            remove_directory_contents_except(&directory, locator_file)?;
        } else if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
            removed_ancestor.push(directory);
        }
    }
    remove_common_root_if_empty(previous);
    Ok(())
}

fn cleanup_target(target: &AppPaths, default: &AppPaths, locator_file: &Path) -> CoreResult<()> {
    let mut directories = vec![
        target.config_dir.clone(),
        target.data_dir.clone(),
        target.log_dir.clone(),
    ];
    directories.sort_by_key(|path| path.components().count());
    let mut removed_ancestor: Vec<PathBuf> = Vec::new();
    for directory in directories {
        if removed_ancestor
            .iter()
            .any(|parent| directory.starts_with(parent))
        {
            continue;
        }
        if directory == default.config_dir {
            remove_directory_contents_except(&directory, locator_file)?;
        } else if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
            removed_ancestor.push(directory);
        }
    }
    remove_common_root_if_empty(target);
    Ok(())
}

fn remove_directory_contents_except(directory: &Path, keep: &Path) -> CoreResult<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path == keep {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn remove_common_root_if_empty(paths: &AppPaths) {
    let Some(root) = paths.config_dir.parent() else {
        return;
    };
    if paths.data_dir.parent() == Some(root) && paths.log_dir.parent() == Some(root) {
        let _ = std::fs::remove_dir(root);
    }
}

fn launcher_directory() -> CoreResult<PathBuf> {
    if let Some(app_image) = std::env::var_os("APPIMAGE") {
        let path = PathBuf::from(app_image);
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| CoreError::InvalidConfig("application directory is unavailable".into()))?
        .to_path_buf();
    #[cfg(target_os = "macos")]
    let directory = if directory.file_name().is_some_and(|name| name == "MacOS") {
        if let Some(app_parent) = directory
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
        {
            app_parent.to_path_buf()
        } else {
            directory
        }
    } else {
        directory
    };
    Ok(directory)
}

fn default_root(paths: &AppPaths) -> PathBuf {
    paths.config_dir.clone()
}

fn storage_info(
    configured: bool,
    mode: StorageMode,
    root: &Path,
    paths: &AppPaths,
    locator_file: &Path,
) -> StorageInfo {
    StorageInfo {
        configured,
        mode,
        root: root.display().to_string(),
        config_dir: paths.config_dir.display().to_string(),
        data_dir: paths.data_dir.display().to_string(),
        log_dir: paths.log_dir.display().to_string(),
        locator_file: locator_file.display().to_string(),
    }
}

fn verify_writable(directory: &Path) -> CoreResult<()> {
    let temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.as_file().sync_all()?;
    Ok(())
}

fn atomic_json(path: &Path, value: &impl Serialize) -> CoreResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::InvalidConfig("invalid storage configuration path".into()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(value)?)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_root_uses_contained_directories() {
        let root = PathBuf::from("/srv/codex-notify");
        let paths = AppPaths::from_root(&root);
        assert_eq!(paths.config_dir, root.join("config"));
        assert_eq!(paths.data_dir, root.join("data"));
        assert_eq!(paths.log_dir, root.join("logs"));
    }

    #[test]
    fn locator_round_trips_windows_style_paths() {
        let locator = StorageLocator {
            schema_version: 1,
            mode: StorageMode::Custom,
            root: Some(PathBuf::from(r"C:\Users\Alice\CodexNotifyData")),
        };
        let json = serde_json::to_vec(&locator).unwrap();
        let decoded: StorageLocator = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.root, locator.root);
    }

    #[test]
    fn migration_copies_database_and_removes_previous_directories() {
        let source_root = tempfile::tempdir().unwrap();
        let target_root = tempfile::tempdir().unwrap();
        let default_root = tempfile::tempdir().unwrap();
        let source = AppPaths::from_root(source_root.path().join("old"));
        let target = AppPaths::from_root(target_root.path().join("new"));
        let default = AppPaths::from_root(default_root.path().join("system"));
        source.ensure().unwrap();
        target.ensure().unwrap();
        std::fs::write(source.settings_file(), b"settings").unwrap();
        std::fs::write(source.log_dir.join("desktop.log"), b"log").unwrap();
        let database = rusqlite::Connection::open(source.events_db()).unwrap();
        database
            .execute_batch("CREATE TABLE sample(value TEXT); INSERT INTO sample VALUES ('ok');")
            .unwrap();
        drop(database);

        copy_application_data(&source, &target, &default.config_dir.join(STORAGE_FILE)).unwrap();
        validate_migrated_data(&source, &target).unwrap();
        let copied: String = rusqlite::Connection::open(target.events_db())
            .unwrap()
            .query_row("SELECT value FROM sample", [], |row| row.get(0))
            .unwrap();
        assert_eq!(copied, "ok");
        assert_eq!(std::fs::read(target.settings_file()).unwrap(), b"settings");
        assert_eq!(
            std::fs::read(target.log_dir.join("desktop.log")).unwrap(),
            b"log"
        );

        cleanup_previous(
            &source,
            &target,
            &default,
            &default.config_dir.join(STORAGE_FILE),
        )
        .unwrap();
        assert!(!source.config_dir.exists());
        assert!(!source.data_dir.exists());
        assert!(!source.log_dir.exists());
    }
}

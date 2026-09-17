use crate::session::{Session, Sessions};
use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STORE_VERSION: u32 = 1;
const STORE_FILE: &str = "sessions.json";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoreDocument {
    version: u32,
    active_session_id: u64,
    next_session_id: u64,
    sessions: Vec<Session>,
}

pub(crate) struct SessionStorage {
    path: PathBuf,
}

impl SessionStorage {
    pub(crate) fn discover() -> Result<Self> {
        let directories = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
            .context("platform local data directory is unavailable")?;
        Ok(Self {
            path: directories.data_local_dir().join(STORE_FILE),
        })
    }

    pub(crate) fn load(&self) -> Result<Option<Sessions>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read session store {}", self.path.display())
                });
            }
        };
        let document: StoreDocument = serde_json::from_slice(&bytes)
            .with_context(|| format!("malformed session store {}", self.path.display()))?;
        if document.version != STORE_VERSION {
            bail!(
                "unsupported session store version {} in {}; expected {}",
                document.version,
                self.path.display(),
                STORE_VERSION
            );
        }
        let sessions = Sessions::restore(
            document.sessions,
            document.active_session_id,
            document.next_session_id,
        )
        .with_context(|| format!("invalid session store {}", self.path.display()))?;
        Ok(Some(sessions))
    }

    pub(crate) fn save(&self, sessions: &Sessions) -> Result<()> {
        let document = StoreDocument {
            version: STORE_VERSION,
            active_session_id: sessions.active_id(),
            next_session_id: sessions.next_id(),
            sessions: sessions.items().to_vec(),
        };
        let bytes =
            serde_json::to_vec_pretty(&document).context("failed to serialize session store")?;
        let parent = self
            .path
            .parent()
            .context("session store path has no parent")?;
        create_private_directory(parent)?;
        let (temporary_path, mut temporary) = create_temporary_sibling(&self.path)?;

        let write_result = (|| -> Result<()> {
            temporary
                .write_all(&bytes)
                .context("failed to write temporary session store")?;
            temporary
                .write_all(b"\n")
                .context("failed to finish temporary session store")?;
            temporary
                .sync_all()
                .context("failed to sync temporary session store")?;
            drop(temporary);
            fs::rename(&temporary_path, &self.path).with_context(|| {
                format!(
                    "failed to atomically replace session store {}",
                    self.path.display()
                )
            })?;
            #[cfg(unix)]
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .context("failed to sync session store directory")?;
            Ok(())
        })();

        if let Err(error) = write_result {
            return match fs::remove_file(&temporary_path) {
                Ok(()) => Err(error),
                Err(cleanup) if cleanup.kind() == ErrorKind::NotFound => Err(error),
                Err(cleanup) => Err(anyhow!(
                    "{error:#}; failed to remove temporary store {}: {cleanup}",
                    temporary_path.display()
                )),
            };
        }
        Ok(())
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

fn create_private_directory(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .with_context(|| format!("failed to create session data directory {}", path.display()))
}

fn create_temporary_sibling(path: &Path) -> Result<(PathBuf, File)> {
    let parent = path.parent().context("session store path has no parent")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("session store filename is not valid Unicode")?;
    for _ in 0..16 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary store {}", candidate.display())
                });
            }
        }
    }
    bail!("could not allocate a temporary session store file")
}

#[cfg(test)]
mod tests;

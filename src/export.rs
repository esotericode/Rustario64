//! Private directory exports: write completely before publishing, and serialize
//! concurrent exporters so an existing destination is never replaced.
use crate::{AppResult, write_new};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

struct StagingDirectory(PathBuf);

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        // After a successful rename the old staging path no longer exists.
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn directory_new(
    destination: &Path,
    write_contents: impl FnOnce(&Path) -> AppResult<()>,
) -> AppResult<()> {
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    // OS releases the lock on errors, panics, or process exit. The empty lock
    // file stays in the export parent; removing it would split concurrent locks.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(".rustario64-export.lock"))?;
    lock.lock()?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "export already exists").into(),
            );
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let staging = loop {
        let id = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".rustario64-import-{}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => break StagingDirectory(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    };
    write_new(
        &staging.0.join(".gitignore"),
        b"# Private ROM-derived export\n*\n",
    )?;
    write_contents(&staging.0)?;
    fs::rename(&staging.0, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> StagingDirectory {
        let path = Path::new("target").join(format!(
            "export-test-{}-{}",
            std::process::id(),
            NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        StagingDirectory(path)
    }

    #[test]
    fn failed_export_cleans_up_and_can_be_retried() {
        let root = root();
        let destination = root.0.join("bob");
        let result = directory_new(&destination, |stage| {
            write_new(&stage.join("first.json"), b"partial")?;
            Err(io::Error::other("injected write failure").into())
        });
        assert!(result.is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1); // Only the lock.
        directory_new(&destination, |stage| {
            assert!(!destination.exists());
            write_new(&stage.join("first.json"), b"complete")
        })
        .unwrap();
        assert_eq!(
            fs::read(destination.join("first.json")).unwrap(),
            b"complete"
        );
        assert!(destination.join(".gitignore").exists());
    }

    #[test]
    fn existing_file_or_directory_is_preserved() {
        let root = root();
        for directory in [false, true] {
            let destination = root.0.join(if directory { "directory" } else { "file" });
            if directory {
                fs::create_dir(&destination).unwrap();
                fs::write(destination.join("sentinel"), b"original").unwrap();
            } else {
                fs::write(&destination, b"original").unwrap();
            }
            assert!(directory_new(&destination, |_| panic!("must not write")).is_err());
            assert_eq!(
                fs::read(if directory {
                    destination.join("sentinel")
                } else {
                    destination
                })
                .unwrap(),
                b"original"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn dangling_destination_symlink_is_preserved() {
        let root = root();
        let destination = root.0.join("bob");
        std::os::unix::fs::symlink("missing-target", &destination).unwrap();
        assert!(directory_new(&destination, |_| panic!("must not write")).is_err());
        assert_eq!(
            fs::read_link(destination).unwrap(),
            Path::new("missing-target")
        );
    }

    #[test]
    fn concurrent_exporters_publish_only_one_complete_result() {
        let root = root();
        let destination = root.0.join("bob");
        let results = std::thread::scope(|scope| {
            let publish = |bytes: &'static [u8]| {
                directory_new(&destination, |stage| {
                    write_new(&stage.join("marker"), bytes)
                })
                .is_ok()
            };
            let a = scope.spawn(move || publish(b"a"));
            let b = scope.spawn(move || publish(b"b"));
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_eq!(results.into_iter().filter(|&ok| ok).count(), 1);
        assert!(matches!(
            fs::read(destination.join("marker")).unwrap().as_slice(),
            b"a" | b"b"
        ));
    }
}

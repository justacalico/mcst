//! Single-instance lock: refuses a second mcst against the same data dir.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

pub struct SingleInstance {
    path: PathBuf,
    _file: File,
}

impl SingleInstance {
    /// Acquire `<data_dir>/mcst.lock`. Fails if another live process holds it.
    pub fn acquire(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("mcst.lock");
        std::fs::create_dir_all(data_dir)?;
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)?;
        // POSIX advisory lock where available.
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if rc != 0 {
                bail!(
                    "another mcst instance is already running for {} — stop it first or use -d to pick a different data directory",
                    data_dir.display()
                );
            }
        }
        f.set_len(0)?;
        writeln!(f, "{}", std::process::id())?;
        Ok(Self { path, _file: f })
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_blocks_second_holder() {
        let d = tempfile::tempdir().unwrap();
        let a = SingleInstance::acquire(d.path()).unwrap();
        let b = SingleInstance::acquire(d.path());
        // On unix flock blocks; on other platforms the second open may pass.
        #[cfg(unix)]
        assert!(b.is_err());
        drop(a);
        assert!(SingleInstance::acquire(d.path()).is_ok());
    }
}

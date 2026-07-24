use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct ArtifactReservation {
    pub raw_path: PathBuf,
    pub summary_path: PathBuf,
    pub raw_file: File,
    pub summary_file: File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultIdentity {
    pub unix_milliseconds: u128,
    pub process_id: u32,
}

impl DefaultIdentity {
    pub fn current() -> Result<Self, String> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?;
        Ok(Self {
            unix_milliseconds: elapsed.as_millis(),
            process_id: std::process::id(),
        })
    }
}

pub fn reserve_explicit(raw_path: PathBuf) -> Result<ArtifactReservation, String> {
    create_parent(&raw_path)?;
    let summary_path = summary_path(&raw_path);
    create_parent(&summary_path)?;
    reserve_pair(raw_path, summary_path).map_err(|failure| failure.message)
}

pub fn reserve_default(
    directory: &Path,
    identity: DefaultIdentity,
) -> Result<ArtifactReservation, String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "could not create default trace directory {}: {error}",
            directory.display()
        )
    })?;

    for suffix in 0_u64.. {
        let suffix = if suffix == 0 {
            String::new()
        } else {
            format!("-{suffix}")
        };
        let raw_path = directory.join(format!(
            "playtest-{}-p{}{}.ronl",
            identity.unix_milliseconds, identity.process_id, suffix
        ));
        let summary_path = summary_path(&raw_path);
        match reserve_pair(raw_path, summary_path) {
            Ok(reservation) => return Ok(reservation),
            Err(failure) if failure.collision => continue,
            Err(failure) => return Err(failure.message),
        }
    }
    unreachable!("the numeric collision suffix cannot be exhausted")
}

#[must_use]
pub fn summary_path(raw_path: &Path) -> PathBuf {
    raw_path.with_extension("summary.ron")
}

fn create_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create artifact directory {}: {error}",
            parent.display()
        )
    })
}

struct ReservationFailure {
    collision: bool,
    message: String,
}

fn reserve_pair(
    raw_path: PathBuf,
    summary_path: PathBuf,
) -> Result<ArtifactReservation, ReservationFailure> {
    let raw_file = create_new(&raw_path).map_err(|error| ReservationFailure {
        collision: error.kind() == io::ErrorKind::AlreadyExists,
        message: create_error("trace", &raw_path, &error),
    })?;

    match create_new(&summary_path) {
        Ok(summary_file) => Ok(ArtifactReservation {
            raw_path,
            summary_path,
            raw_file,
            summary_file,
        }),
        Err(error) => {
            drop(raw_file);
            let cleanup_error = fs::remove_file(&raw_path).err();
            let mut message = create_error("summary", &summary_path, &error);
            if let Some(cleanup_error) = &cleanup_error {
                message.push_str(&format!(
                    "; also could not remove newly reserved trace {}: {cleanup_error}",
                    raw_path.display()
                ));
            }
            Err(ReservationFailure {
                collision: error.kind() == io::ErrorKind::AlreadyExists && cleanup_error.is_none(),
                message,
            })
        }
    }
}

fn create_new(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn create_error(kind: &str, path: &Path, error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::AlreadyExists {
        format!(
            "refusing to overwrite existing {kind} artifact {}",
            path.display()
        )
    } else {
        format!(
            "could not create {kind} artifact {}: {error}",
            path.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("4x-term-game-play-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn explicit_reservation_creates_parents_and_both_artifacts() {
        let directory = TestDirectory::new();
        let raw_path = directory.0.join("nested/session.ronl");
        let reservation = reserve_explicit(raw_path.clone()).unwrap();

        assert_eq!(reservation.raw_path, raw_path);
        assert_eq!(
            reservation.summary_path,
            directory.0.join("nested/session.summary.ron")
        );
        assert!(reservation.raw_path.exists());
        assert!(reservation.summary_path.exists());
    }

    #[test]
    fn explicit_reservation_never_overwrites_either_artifact() {
        let directory = TestDirectory::new();
        let raw_path = directory.0.join("session.ronl");
        fs::write(&raw_path, "existing evidence").unwrap();
        assert!(reserve_explicit(raw_path.clone()).is_err());
        assert_eq!(fs::read_to_string(&raw_path).unwrap(), "existing evidence");

        fs::remove_file(&raw_path).unwrap();
        let summary = summary_path(&raw_path);
        fs::write(&summary, "existing summary").unwrap();
        assert!(reserve_explicit(raw_path.clone()).is_err());
        assert!(!raw_path.exists());
        assert_eq!(fs::read_to_string(summary).unwrap(), "existing summary");
    }

    #[test]
    fn default_collisions_receive_a_monotonic_suffix() {
        let directory = TestDirectory::new();
        let identity = DefaultIdentity {
            unix_milliseconds: 1_234,
            process_id: 56,
        };
        let first = reserve_default(&directory.0, identity).unwrap();
        let mut first_raw = first.raw_file;
        writeln!(first_raw, "do not overwrite").unwrap();
        drop(first_raw);
        drop(first.summary_file);

        let second = reserve_default(&directory.0, identity).unwrap();
        assert_eq!(
            second.raw_path.file_name().unwrap(),
            "playtest-1234-p56-1.ronl"
        );
        assert_eq!(
            fs::read_to_string(first.raw_path).unwrap(),
            "do not overwrite\n"
        );
    }

    #[test]
    fn a_default_summary_collision_also_advances_the_suffix() {
        let directory = TestDirectory::new();
        let identity = DefaultIdentity {
            unix_milliseconds: 9,
            process_id: 7,
        };
        fs::write(directory.0.join("playtest-9-p7.summary.ron"), "keep").unwrap();

        let reservation = reserve_default(&directory.0, identity).unwrap();
        assert_eq!(
            reservation.raw_path.file_name().unwrap(),
            "playtest-9-p7-1.ronl"
        );
        assert!(!directory.0.join("playtest-9-p7.ronl").exists());
    }
}

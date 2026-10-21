// SPDX-License-Identifier: MIT
use super::*;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        for _ in 0..16 {
            let mut random = [0; 16];
            getrandom::fill(&mut random).unwrap();
            let path = std::env::temp_dir().join(format!(
                "cobble-stage-{}-{}",
                std::process::id(),
                hex(&random)
            ));
            if std::fs::create_dir(&path).is_ok() {
                return Self(path);
            }
        }
        panic!("test directory allocation failed");
    }
    fn staging(&self) -> DownloadStaging {
        DownloadStaging::open(Dir::open_ambient_dir(&self.0, cap_std::ambient_authority()).unwrap())
            .unwrap()
    }
    fn job(&self) -> PathBuf {
        self.0.join("jobs").join(hex(&identity().request_key))
    }
    fn part(&self) -> PathBuf {
        self.job().join(PART)
    }
    fn snapshot(&self) -> PathBuf {
        self.job().join(SNAPSHOT)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn identity() -> StageIdentity {
    StageIdentity {
        request_key: [7; 32],
        expected_size: 6,
        max_bytes: 6,
    }
}
fn prefix(temp: &Temp) {
    let mut stage = temp.staging().open_job(identity()).unwrap();
    stage.write_all(b"abc").unwrap();
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
}
fn assert_resume(stage: &DownloadStage, offset: u64) {
    let StageRecovery::Resume(checkpoint) = stage.recovery() else {
        panic!("durable prefix should resume");
    };
    assert_eq!(checkpoint.durable_offset, offset);
    assert_eq!(checkpoint.validator.as_deref(), Some(b"\"v1\"".as_slice()));
}

#[test]
fn stage_sqlite_snapshot_and_same_inode_recovery_truncate_unpublished_tail() {
    // Normal appends + repeated checkpoints never reread the previous prefix.
    let linear = Temp::new();
    let mut stage = linear.staging().open_job(identity()).unwrap();
    for byte in b"abc" {
        stage.write_all(&[*byte]).unwrap();
        stage.checkpoint(Some(b"\"v1\"")).unwrap();
        assert_eq!(stage.prefix_bytes_read, 0);
    }
    drop(stage);
    let mut stage = linear.staging().open_job(identity()).unwrap();
    assert_eq!(
        stage.prefix_bytes_read, 3,
        "restart must actually rehash once"
    );
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    assert_eq!(stage.prefix_bytes_read, 3);
    // CAS-style reads/seeks do not invalidate the cache. A nonappend write
    // does, and exactly one next checkpoint rebuilds from the actual file.
    stage.seek(SeekFrom::Start(0)).unwrap();
    let mut first = [0];
    stage.read_exact(&mut first).unwrap();
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    assert_eq!(stage.prefix_bytes_read, 3);
    stage.seek(SeekFrom::Start(0)).unwrap();
    stage.write_all(b"A").unwrap();
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    assert_eq!(stage.prefix_bytes_read, 6);
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    assert_eq!(stage.prefix_bytes_read, 6);
    stage.reset_for_full(None).unwrap();
    stage.write_all(b"new").unwrap();
    stage.checkpoint(None).unwrap();
    assert_eq!(
        stage.prefix_bytes_read, 6,
        "reset starts a new incremental hash"
    );

    let temp = Temp::new();
    prefix(&temp);
    let mut stage = temp.staging().open_job(identity()).unwrap();
    assert_resume(&stage, 3);
    stage.write_all(b"def").unwrap();
    assert_eq!(stage.byte_len().unwrap(), 6);
    drop(stage);
    let mut stage = temp.staging().open_job(identity()).unwrap();
    assert_resume(&stage, 3);
    assert_eq!(stage.byte_len().unwrap(), 3);
    assert_eq!(std::fs::read(temp.part()).unwrap(), b"abc");
    stage.write_all(b"def").unwrap();
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    stage.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    stage.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"abcdef");
    drop(stage);
    assert_resume(&temp.staging().open_job(identity()).unwrap(), 6);
    let image = std::fs::read(temp.snapshot()).unwrap();
    assert!(image.len() <= IMAGE_BYTES);
    validate_image_profile(&image).unwrap();
}

#[test]
fn stage_short_or_bad_prefix_restarts_full_and_validated_identity_is_required() {
    for bytes in [b"ab".as_slice(), b"xyz".as_slice()] {
        let temp = Temp::new();
        prefix(&temp);
        std::fs::write(temp.part(), bytes).unwrap();
        let stage = temp.staging().open_job(identity()).unwrap();
        assert!(matches!(stage.recovery(), StageRecovery::Full));
        assert_eq!(stage.byte_len().unwrap(), 0);
    }
    let temp = Temp::new();
    prefix(&temp);
    let mut other = identity();
    other.max_bytes = 7;
    assert!(matches!(
        temp.staging().open_job(other),
        Err(StageError::IdentityMismatch)
    ));
    assert_eq!(std::fs::read(temp.part()).unwrap(), b"abc");
}

fn alter_snapshot(temp: &Temp, sql: &str) {
    let image = std::fs::read(temp.snapshot()).unwrap();
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .deserialize_read_exact(MAIN_DB, io::Cursor::new(&image), image.len(), false)
        .unwrap();
    connection.execute_batch(sql).unwrap();
    std::fs::write(temp.snapshot(), &*connection.serialize(MAIN_DB).unwrap()).unwrap();
}
#[test]
fn stage_closed_sqlite_schema_version_rows_and_header_profile_fail_closed() {
    for sql in [
        "PRAGMA user_version=2",
        "CREATE VIEW untrusted AS SELECT * FROM checkpoint",
        "CREATE TABLE extra(value BLOB)",
        "UPDATE checkpoint SET durable_offset=7",
        "UPDATE checkpoint SET validator=zeroblob(1025)",
        "CREATE TRIGGER untrusted AFTER UPDATE ON checkpoint BEGIN SELECT 1; END",
    ] {
        let temp = Temp::new();
        prefix(&temp);
        alter_snapshot(&temp, sql);
        assert!(
            temp.staging().open_job(identity()).is_err(),
            "closed snapshot accepted mutation"
        );
        assert_eq!(std::fs::read(temp.part()).unwrap(), b"abc");
    }
    for (at, bytes) in [
        (28, vec![0xff; 4]),
        (18, vec![2, 2]),
        (56, vec![0, 0, 0, 2]),
        (0, vec![0]),
    ] {
        let temp = Temp::new();
        prefix(&temp);
        let mut image = std::fs::read(temp.snapshot()).unwrap();
        image[at..at + bytes.len()].copy_from_slice(&bytes);
        std::fs::write(temp.snapshot(), image).unwrap();
        assert!(matches!(
            temp.staging().open_job(identity()),
            Err(StageError::InvalidCheckpoint)
        ));
    }
    for size in [0, 1, IMAGE_BYTES + 1] {
        let temp = Temp::new();
        prefix(&temp);
        std::fs::write(temp.snapshot(), vec![0; size]).unwrap();
        assert!(matches!(
            temp.staging().open_job(identity()),
            Err(StageError::InvalidCheckpoint)
        ));
    }
}

#[test]
fn stage_limits_exclusive_owner_cleanup_and_scoped_names() {
    let temp = Temp::new();
    let mut staging = temp.staging();
    let mut stage = staging.open_job(identity()).unwrap();
    assert!(matches!(
        staging.open_job(identity()),
        Err(StageError::Busy)
    ));
    assert!(matches!(
        staging.cleanup_stale_snapshots_exclusive(),
        Err(StageError::Busy)
    ));
    assert!(stage.write_all(b"abcdef!").is_err());
    assert_eq!(stage.byte_len().unwrap(), 0);
    assert!(stage.seek(SeekFrom::Start(7)).is_err());
    assert!(stage.checkpoint(Some(&[0; 1025])).is_err());
    let mut invalid = identity();
    invalid.max_bytes = HARD_BYTES + 1;
    assert!(matches!(
        staging.open_job(invalid),
        Err(StageError::InvalidIdentity)
    ));
    drop(stage);
    let stale = format!("checkpoint-{}.tmp", "a".repeat(32));
    std::fs::write(temp.job().join(&stale), b"partial image").unwrap();
    std::fs::write(temp.job().join("user-file.tmp"), b"keep").unwrap();
    assert_eq!(staging.cleanup_stale_snapshots_exclusive().unwrap(), 1);
    assert_eq!(
        std::fs::read(temp.job().join("user-file.tmp")).unwrap(),
        b"keep"
    );
    assert!(temp.snapshot().exists());
}

struct Redirect(PathBuf);
impl Drop for Redirect {
    fn drop(&mut self) {
        #[cfg(windows)]
        let _ = std::fs::remove_dir(&self.0);
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.0);
    }
}
fn redirect(target: &Path, link: &Path) -> Redirect {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "test junction creation failed");
    }
    Redirect(link.to_owned())
}
#[test]
fn stage_reparse_and_hardlinked_objects_cannot_touch_outside_files() {
    let temp = Temp::new();
    let outside = Temp::new();
    let staging = temp.staging();
    std::fs::write(outside.0.join("sentinel"), b"outside").unwrap();
    let _redirect = redirect(&outside.0, &temp.job());
    assert!(staging.open_job(identity()).is_err());
    assert_eq!(
        std::fs::read(outside.0.join("sentinel")).unwrap(),
        b"outside"
    );
    drop(_redirect);
    for name in [PART, SNAPSHOT, ".job.lock"] {
        let temp = Temp::new();
        prefix(&temp);
        let file = outside.0.join(format!("outside-{name}"));
        std::fs::write(&file, b"outside").unwrap();
        std::fs::remove_file(temp.job().join(name)).unwrap();
        std::fs::hard_link(&file, temp.job().join(name)).unwrap();
        assert!(matches!(
            temp.staging().open_job(identity()),
            Err(StageError::UnsafeObject)
        ));
        assert_eq!(std::fs::read(&file).unwrap(), b"outside");
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn stage_child() {
    let Some(path) = std::env::var_os("COBBLE_STAGE_CHILD_ROOT") else {
        return;
    };
    let staging =
        DownloadStaging::open(Dir::open_ambient_dir(path, cap_std::ambient_authority()).unwrap())
            .unwrap();
    let mut stage = staging.open_job(identity()).unwrap();
    if std::env::var("COBBLE_STAGE_CHILD_PHASE").unwrap() == "full_reset" {
        stage.reset_for_full(Some(b"\"v2\"")).unwrap();
    } else {
        stage.write_all(b"def").unwrap();
        stage.checkpoint(Some(b"\"v1\"")).unwrap();
    }
    panic!("fault point not reached");
}
#[test]
fn stage_real_process_crashes_recover_only_published_synced_prefixes() {
    for phase in [
        "partial_written",
        "partial_synced",
        "image_synced",
        "checkpoint_published",
        "full_reset",
    ] {
        let temp = Temp::new();
        prefix(&temp);
        let signal = temp.0.join("child-ready");
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "staging::tests::stage_child", "--nocapture"])
                .env("COBBLE_STAGE_CHILD_ROOT", &temp.0)
                .env("COBBLE_STAGE_CHILD_PHASE", phase)
                .env("COBBLE_STAGE_CHILD_SIGNAL", &signal)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !signal.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before fault point"
            );
            assert!(Instant::now() < deadline, "child fault point deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut staging = temp.staging();
        assert!(matches!(
            staging.open_job(identity()),
            Err(StageError::Busy)
        ));
        assert!(matches!(
            staging.cleanup_stale_snapshots_exclusive(),
            Err(StageError::Busy)
        ));
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let stage = staging.open_job(identity()).unwrap();
        match phase {
            "checkpoint_published" => {
                assert_resume(&stage, 6);
                assert_eq!(stage.byte_len().unwrap(), 6);
            }
            "full_reset" => {
                assert!(matches!(stage.recovery(), StageRecovery::Full));
                assert_eq!(stage.byte_len().unwrap(), 0);
            }
            _ => {
                assert_resume(&stage, 3);
                assert_eq!(stage.byte_len().unwrap(), 3);
            }
        }
        drop(stage);
        assert_eq!(
            staging.cleanup_stale_snapshots_exclusive().unwrap(),
            u32::from(phase == "image_synced")
        );
    }
}

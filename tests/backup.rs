mod common;
use common::helpers::unique_temp_dir;

use std::fs;

use anvil::backup::*;
use anvil::error::Error;

const TEST_VERSION: &str = "1.21";

#[test]
fn no_backups_in_empty_dir() {
    let dir = unique_temp_dir("backup-no-backups");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let result = find_latest_backup(&dir);
    assert!(matches!(result, Err(Error::NoBackups)));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn finds_latest_backup() {
    let dir = unique_temp_dir("backup-find");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let older = dir.join("backup_01-01-2024_mc1.21");
    let newer = dir.join("backup_02-01-2024_mc1.21");
    fs::create_dir(&older).unwrap();
    // Touch a file inside older so it gets an earlier mtime.
    fs::write(older.join("dummy"), b"").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::create_dir(&newer).unwrap();

    let found = find_latest_backup(&dir).unwrap();
    assert_eq!(found, newer);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_backup_dir_creates_directory() {
    let dir = unique_temp_dir("backup-create");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup = create_backup_dir(&dir, TEST_VERSION).unwrap();
    assert!(backup.exists());
    assert!(backup.is_dir());
    assert!(backup
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("backup_"));

    let _ = fs::remove_dir_all(&dir);
}

// ── move_to_backup ───────────────────────────────────────────────────

#[test]
fn move_to_backup_moves_not_copies() {
    let dir = unique_temp_dir("backup-move");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup_dir = dir.join("backup_01-01-2024_mc1.21");
    fs::create_dir(&backup_dir).unwrap();

    let jar_path = dir.join("test-mod.jar");
    fs::write(&jar_path, b"fake jar content").unwrap();

    move_to_backup(&jar_path, &backup_dir).unwrap();

    // Original should be gone (move, not copy)
    assert!(!jar_path.exists());

    // Backup should contain the file with correct content
    let backup_jar = backup_dir.join("test-mod.jar");
    assert!(backup_jar.exists());
    let content = fs::read_to_string(&backup_jar).unwrap();
    assert_eq!(content, "fake jar content");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn move_to_backup_multiple_files() {
    let dir = unique_temp_dir("backup-move-multiple");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup_dir = dir.join("backup_01-01-2024_mc1.21");
    fs::create_dir(&backup_dir).unwrap();

    let jars: Vec<_> = (0..5)
        .map(|i| {
            let path = dir.join(format!("mod-{}.jar", i));
            fs::write(&path, format!("content-{}", i)).unwrap();
            path
        })
        .collect();

    for jar in &jars {
        move_to_backup(jar, &backup_dir).unwrap();
    }

    // All originals gone
    for jar in &jars {
        assert!(!jar.exists(), "{} should have been moved", jar.display());
    }

    // All in backup with correct content
    for (i, jar) in jars.iter().enumerate() {
        let backup_jar = backup_dir.join(jar.file_name().unwrap());
        assert!(backup_jar.exists());
        let content = fs::read_to_string(&backup_jar).unwrap();
        assert_eq!(content, format!("content-{}", i));
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn move_to_backup_nonexistent_file_errors() {
    let dir = unique_temp_dir("backup-move-nonexistent");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup_dir = dir.join("backup_01-01-2024_mc1.21");
    fs::create_dir(&backup_dir).unwrap();

    let jar_path = dir.join("does-not-exist.jar");
    let result = move_to_backup(&jar_path, &backup_dir);
    assert!(result.is_err());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn move_to_backup_nonexistent_backup_dir_errors() {
    let dir = unique_temp_dir("backup-move-nobackup");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let jar_path = dir.join("mod.jar");
    fs::write(&jar_path, b"content").unwrap();

    let fake_backup_dir = dir.join("backup_does_not_exist");
    let result = move_to_backup(&jar_path, &fake_backup_dir);
    assert!(result.is_err());

    // Original file should still exist (move failed)
    assert!(jar_path.exists());

    let _ = fs::remove_dir_all(&dir);
}

// ── rollback (round-trip) ───────────────────────────────────────────

#[test]
fn rollback_full_round_trip() {
    let dir = unique_temp_dir("backup-rollback");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // Create mod files
    let jar1 = dir.join("mod-a.jar");
    let jar2 = dir.join("mod-b.jar");
    let jar3 = dir.join("mod-c.jar");
    fs::write(&jar1, b"mod-a-content").unwrap();
    fs::write(&jar2, b"mod-b-content").unwrap();
    fs::write(&jar3, b"mod-c-content").unwrap();

    // Create backup and move files into it
    let backup = create_backup_dir(&dir, TEST_VERSION).unwrap();
    move_to_backup(&jar1, &backup).unwrap();
    move_to_backup(&jar2, &backup).unwrap();
    move_to_backup(&jar3, &backup).unwrap();

    // All originals gone
    assert!(!jar1.exists());
    assert!(!jar2.exists());
    assert!(!jar3.exists());

    // Rollback: restore from backup
    let count = rollback(&dir).unwrap();
    assert_eq!(count, 3);

    // All files restored
    assert!(jar1.exists());
    assert!(jar2.exists());
    assert!(jar3.exists());

    // Content preserved
    assert_eq!(fs::read_to_string(&jar1).unwrap(), "mod-a-content");
    assert_eq!(fs::read_to_string(&jar2).unwrap(), "mod-b-content");
    assert_eq!(fs::read_to_string(&jar3).unwrap(), "mod-c-content");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rollback_overwrites_existing_files() {
    let dir = unique_temp_dir("backup-rollback-overwrite");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // Create original file and move to backup
    let jar = dir.join("mod.jar");
    fs::write(&jar, b"original").unwrap();

    let backup = create_backup_dir(&dir, TEST_VERSION).unwrap();
    move_to_backup(&jar, &backup).unwrap();

    // Create a "newer" file at the same path (simulates downloaded update)
    fs::write(&jar, b"newer version").unwrap();

    // Rollback should first save the "newer" file to a safety backup,
    // then restore the original from the backup.
    let count = rollback(&dir).unwrap();
    assert_eq!(count, 1);
    assert_eq!(fs::read_to_string(&jar).unwrap(), "original");

    // Verify a safety backup was created containing the newer file.
    let safety_dirs: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("backup_before_rollback_"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(safety_dirs.len(), 1);
    let safety_jar = safety_dirs[0].path().join("mod.jar");
    assert!(safety_jar.exists());
    assert_eq!(fs::read_to_string(&safety_jar).unwrap(), "newer version");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rollback_no_backups_errors() {
    let dir = unique_temp_dir("backup-rollback-none");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let result = rollback(&dir);
    assert!(result.is_err());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rollback_empty_backup_returns_zero() {
    let dir = unique_temp_dir("backup-rollback-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup = create_backup_dir(&dir, TEST_VERSION).unwrap();
    assert!(backup.exists());

    let count = rollback(&dir).unwrap();
    assert_eq!(count, 0);

    let _ = fs::remove_dir_all(&dir);
}

// ── find_latest_backup with non-backup directories ──────────────────

#[test]
fn find_latest_backup_ignores_non_backup_dirs() {
    let dir = unique_temp_dir("backup-ignore-nonbackup");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // Create a non-backup directory
    fs::create_dir(dir.join("some_other_dir")).unwrap();
    // Create a file that starts with backup_ (should be ignored — not a dir)
    fs::write(dir.join("backup_fake_file"), b"not a dir").unwrap();

    let result = find_latest_backup(&dir);
    assert!(matches!(result, Err(Error::NoBackups)));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_latest_backup_multiple_returns_newest() {
    let dir = unique_temp_dir("backup-newest");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let b1 = dir.join("backup_01-01-2024_mc1.21");
    let b2 = dir.join("backup_02-01-2024_mc1.21");
    let b3 = dir.join("backup_03-01-2024_mc1.21");
    fs::create_dir(&b1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::create_dir(&b2).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::create_dir(&b3).unwrap();

    let found = find_latest_backup(&dir).unwrap();
    assert_eq!(found, b3);

    let _ = fs::remove_dir_all(&dir);
}

// ── create_backup_dir format ─────────────────────────────────────────

#[test]
fn create_backup_dir_timestamp_format() {
    let dir = unique_temp_dir("backup-timestamp");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup = create_backup_dir(&dir, "1.21.1").unwrap();
    let name = backup.file_name().unwrap().to_str().unwrap();

    // Format: backup_DD-MM-YYYY_mcVERSION
    assert!(name.starts_with("backup_"));
    let after = &name["backup_".len()..];
    // Should match: DD-MM-YYYY_mc1.21.1
    // Verify date chars: positions 0-1 = day, 2 = dash, 3-4 = month, 5 = dash, 6-9 = year
    assert_eq!(after.chars().nth(2), Some('-'));
    assert_eq!(after.chars().nth(5), Some('-'));
    assert_eq!(after.chars().nth(10), Some('_'));
    assert!(after.chars().nth(0).unwrap().is_ascii_digit());
    assert!(after.chars().nth(1).unwrap().is_ascii_digit());
    assert!(after.chars().nth(3).unwrap().is_ascii_digit());
    assert!(after.chars().nth(4).unwrap().is_ascii_digit());
    assert!(after.chars().nth(6).unwrap().is_ascii_digit());
    assert!(after.chars().nth(7).unwrap().is_ascii_digit());
    assert!(after.chars().nth(8).unwrap().is_ascii_digit());
    assert!(after.chars().nth(9).unwrap().is_ascii_digit());
    // Version suffix
    assert_eq!(&after[11..], "mc1.21.1");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_backup_dir_uses_auto_when_version_empty() {
    let dir = unique_temp_dir("backup-auto");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup = create_backup_dir(&dir, "").unwrap();
    let name = backup.file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with("_mcauto"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_backup_dir_is_empty() {
    let dir = unique_temp_dir("backup-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let backup = create_backup_dir(&dir, TEST_VERSION).unwrap();
    let entries: Vec<_> = fs::read_dir(&backup).unwrap().collect();
    assert!(entries.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

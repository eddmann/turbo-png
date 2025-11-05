use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::fixture::{PathChild, PathCreateDir};

mod fixtures;

#[test]
fn cli_errors_on_unwritable_output_directory() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let ro_dir = temp.child("readonly");
    ro_dir.create_dir_all()?;
    let input = fixtures::write_unoptimized_rgba(&temp, "readonly/input.png", 8, 8);

    let parent = input.parent().expect("input has parent");
    let metadata = fs::metadata(parent)?;
    let mut ro_perms = metadata.permissions();
    ro_perms.set_mode(0o555);
    fs::set_permissions(parent, ro_perms)?;

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "optimize", "--no-progress"])
        .arg(&input)
        .assert()
        .failure();

    let mut restore = metadata.permissions();
    restore.set_mode(0o755);
    fs::set_permissions(parent, restore)?;

    Ok(())
}

#[test]
fn compress_without_overwrite_fails_when_output_exists() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_palette_source(&temp, "dupe.png");
    let output = fixtures::derived_output_path(&input, "_compressed.png");
    fs::write(&output, b"existing").expect("failed to seed output");
    let original_size = fixtures::file_size(&output);

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "compress", "--no-progress"])
        .arg(&input)
        .assert()
        .failure();

    assert_eq!(original_size, fixtures::file_size(&output));

    Ok(())
}

#[test]
fn overwrite_flag_behavior() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_unoptimized_rgba(&temp, "overwrite.png", 12, 12);
    let output = fixtures::derived_output_path(&input, "_optimized.png");

    fs::write(&output, b"placeholder").expect("failed to precreate output");
    let placeholder_size = fixtures::file_size(&output);

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "optimize", "--no-progress"])
        .arg(&input)
        .assert()
        .failure();
    assert_eq!(placeholder_size, fixtures::file_size(&output));

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "optimize", "--no-progress", "--overwrite"])
        .arg(&input)
        .assert()
        .success();

    let new_size = fixtures::file_size(&output);
    assert!(
        new_size != placeholder_size,
        "overwrite should replace the file"
    );

    Ok(())
}

use std::error::Error;

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::fixture::{PathChild, PathCreateDir};
use predicates::prelude::*;

mod fixtures;

#[test]
fn cli_deduplicates_inputs() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let images_dir = temp.child("images");
    images_dir.create_dir_all()?;

    let first = fixtures::write_unoptimized_rgba(&temp, "images/first.png", 10, 10);
    let second = fixtures::write_unoptimized_rgba(&temp, "images/second.png", 8, 12);
    let extra = fixtures::write_unoptimized_rgba(&temp, "extra.png", 6, 6);

    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "optimize", "--no-progress"])
        .arg(&extra)
        .arg(images_dir.path())
        .arg(images_dir.path())
        .assert()
        .success();

    let optimized_first = fixtures::derived_output_path(&first, "_optimized.png");
    let optimized_second = fixtures::derived_output_path(&second, "_optimized.png");
    let optimized_extra = fixtures::derived_output_path(&extra, "_optimized.png");
    assert!(optimized_first.exists());
    assert!(optimized_second.exists());
    assert!(optimized_extra.exists());

    Ok(())
}

#[test]
fn cli_errors_on_missing_input() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let missing = temp.path().join("missing.png");

    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "optimize", "--no-progress"])
        .arg(&missing)
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));

    Ok(())
}

#[test]
fn dry_run_no_write() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_unoptimized_rgba(&temp, "dry.png", 8, 8);

    let compressed_output = fixtures::derived_output_path(&input, "_compressed.png");
    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "compress", "--dry-run", "--no-progress"])
        .arg(&input)
        .assert()
        .success();
    assert!(
        !compressed_output.exists(),
        "dry-run compress should not create output files"
    );

    let optimized_output = fixtures::derived_output_path(&input, "_optimized.png");
    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "optimize", "--dry-run", "--no-progress"])
        .arg(&input)
        .assert()
        .success();
    assert!(
        !optimized_output.exists(),
        "dry-run optimize should not create output files"
    );

    Ok(())
}

use std::error::Error;

use assert_cmd::Command;
use assert_fs::TempDir;
use png::chunk;
use predicates::prelude::*;

mod fixtures;

#[test]
fn optimize_keep_metadata_preserves_chunks() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_fixture(&temp, "meta.png");
    let output = fixtures::derived_output_path(&input, "_optimized.png");

    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "optimize", "--keep-metadata", "--no-progress"])
        .arg(&input)
        .assert()
        .success();

    let chunks = fixtures::chunk_names(&output);
    assert!(chunks.contains(&chunk::tEXt.0));

    Ok(())
}

#[test]
fn quiet_progress_emits_no_spinner_output() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_unoptimized_rgba(&temp, "quiet.png", 8, 8);

    Command::new(assert_cmd::cargo::cargo_bin!("png-compressor"))
        .args(["--mode", "optimize", "--no-progress", "--dry-run"])
        .arg(&input)
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());

    Ok(())
}

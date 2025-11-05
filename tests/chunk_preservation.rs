use std::error::Error;

use assert_cmd::Command;
use assert_fs::TempDir;
use png::chunk;

mod fixtures;

#[test]
fn compress_safe_keeps_display_chunks_only() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_fixture(&temp, "sample.png");
    let output = fixtures::derived_output_path(&input, "_compressed.png");

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "compress", "--no-progress"])
        .arg(&input)
        .assert()
        .success();

    let chunks = fixtures::chunk_names(&output);
    assert!(chunks.contains(&chunk::pHYs.0), "pHYs chunk should remain");
    assert!(
        !chunks.contains(&chunk::tEXt.0),
        "tEXt chunk should be stripped"
    );

    Ok(())
}

#[test]
fn compress_with_keep_metadata_retains_all_chunks() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_fixture(&temp, "sample.png");
    let output = fixtures::derived_output_path(&input, "_compressed.png");

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "compress", "--no-progress", "--keep-metadata"])
        .arg(&input)
        .assert()
        .success();

    let chunks = fixtures::chunk_names(&output);
    assert!(chunks.contains(&chunk::pHYs.0), "pHYs chunk should remain");
    assert!(chunks.contains(&chunk::tEXt.0), "tEXt chunk should remain");

    Ok(())
}

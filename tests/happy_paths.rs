use std::error::Error;
use std::path::PathBuf;

use assert_cmd::Command;
use assert_fs::TempDir;

mod fixtures;

#[test]
fn optimize_smaller_output() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_unoptimized_rgba(&temp, "noisy.png", 32, 32);
    let output = fixtures::derived_output_path(&input, "_optimized.png");

    let original_size = fixtures::file_size(&input);
    let original_pixels = fixtures::decode_rgba(&input);

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "optimize", "--no-progress"])
        .arg(&input)
        .assert()
        .success();

    let optimized_size = fixtures::file_size(&output);
    assert!(
        optimized_size < original_size,
        "expected optimized file ({optimized_size}) smaller than original ({original_size})"
    );

    let optimized_pixels = fixtures::decode_rgba(&output);
    assert_eq!(
        original_pixels, optimized_pixels,
        "optimize must be lossless"
    );

    Ok(())
}

#[test]
fn compress_palette_reduction() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_palette_source(&temp, "palette.png");
    let output = fixtures::derived_output_path(&input, "_compressed.png");

    let original_size = fixtures::file_size(&input);
    let original_pixels = fixtures::decode_rgba(&input);
    let original_unique = fixtures::unique_color_count(&original_pixels);

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "compress", "--quality", "80", "--no-progress"])
        .arg(&input)
        .assert()
        .success();

    let compressed_size = fixtures::file_size(&output);
    assert!(
        compressed_size < original_size,
        "expected compressed file ({compressed_size}) smaller than original ({original_size})"
    );

    let compressed_pixels = fixtures::decode_rgba(&output);
    let compressed_unique = fixtures::unique_color_count(&compressed_pixels);
    assert!(
        compressed_unique <= original_unique,
        "palette compression should not introduce more unique colors"
    );

    let max_diff = fixtures::max_abs_channel_difference(&original_pixels, &compressed_pixels);
    assert!(
        max_diff <= 5,
        "expected limited color drift after compression, observed diff {max_diff}"
    );

    Ok(())
}

#[test]
fn compress_low_quality_clamp_behaves() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let input = fixtures::write_palette_source(&temp, "lowq.png");
    let output = fixtures::derived_output_path(&input, "_compressed.png");

    let original_pixels = fixtures::decode_rgba(&input);

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "compress", "--quality", "20", "--no-progress"])
        .arg(&input)
        .assert()
        .success();

    let compressed_pixels = fixtures::decode_rgba(&output);
    let compressed_unique = fixtures::unique_color_count(&compressed_pixels);
    assert!(
        compressed_unique <= 8,
        "expected aggressive palette reduction"
    );

    let max_diff = fixtures::max_abs_channel_difference(&original_pixels, &compressed_pixels);
    assert!(
        max_diff <= 80,
        "expected bounded deviation at low quality, observed diff {max_diff}"
    );

    Ok(())
}

#[test]
fn threads_option_effect() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let inputs: Vec<PathBuf> = (0..3)
        .map(|i| fixtures::write_unoptimized_rgba(&temp, &format!("batch{i}.png"), 8, 8))
        .collect();

    Command::new(assert_cmd::cargo::cargo_bin!("turbo-png"))
        .args(["--mode", "optimize", "--threads", "1", "--no-progress"])
        .args(inputs.iter().map(|p| p.as_os_str()))
        .assert()
        .success();

    for input in &inputs {
        let output = fixtures::derived_output_path(input, "_optimized.png");
        assert!(output.exists(), "expected output for {}", input.display());
    }

    Ok(())
}

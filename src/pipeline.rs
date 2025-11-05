use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use oxipng::StripChunks;
use tempfile::Builder as TempFileBuilder;

use crate::cli::CommonOptions;

/// Chunks preserved when using oxipng’s `StripChunks::Safe` policy.
/// Keep in sync with `oxipng::display_chunks::DISPLAY_CHUNKS`.
const DISPLAY_CHUNKS: [[u8; 4]; 7] = [
    *b"cICP", *b"iCCP", *b"sRGB", *b"pHYs", *b"acTL", *b"fcTL", *b"fdAT",
];

pub fn build_strip_policy(common: &CommonOptions) -> Result<StripChunks> {
    if common.keep_metadata {
        return Ok(StripChunks::None);
    }

    Ok(StripChunks::Safe)
}

pub fn strip_policy_allows(strip: &StripChunks, name: &[u8; 4]) -> bool {
    match strip {
        StripChunks::None => true,
        StripChunks::Keep(names) => names.contains(name),
        StripChunks::Strip(names) => !names.contains(name),
        StripChunks::Safe => DISPLAY_CHUNKS.contains(name),
        StripChunks::All => false,
    }
}

pub fn derive_output_path(input: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("input file {:?} lacks a valid stem", input))?;
    Ok(parent.join(format!("{stem}{suffix}")))
}

pub fn write_atomic(path: &Path, data: &[u8], overwrite: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("cannot determine parent directory for {}", path.display()))?;

    if !parent.exists() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output directory {}", parent.display()))?;
    }

    let mut temp_file = TempFileBuilder::new()
        .prefix(".png-opt-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .with_context(|| format!("creating temporary file in {}", parent.display()))?;

    temp_file
        .write_all(data)
        .with_context(|| format!("writing temporary output for {}", path.display()))?;

    temp_file
        .flush()
        .with_context(|| format!("flushing temporary file for {}", path.display()))?;

    if overwrite && path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("removing existing file {}", path.display()))?;
    }

    temp_file
        .persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("persisting optimized file {}", path.display()))?;

    Ok(())
}

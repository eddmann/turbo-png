use std::num::NonZeroUsize;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Parser, ValueEnum};

/// Command-line interface definition.
#[derive(Debug, Parser)]
#[command(author, version, about = "High-performance PNG optimizer & compressor", long_about = None)]
pub struct Cli {
    /// One or more PNG file paths (files or directories are expanded).
    #[arg(required = true, value_name = "PATH", num_args = 1..)]
    pub inputs: Vec<PathBuf>,

    /// Processing mode: lossless optimize or quality-balanced compression.
    #[arg(long, value_enum, default_value_t = Mode::Optimize)]
    pub mode: Mode,

    /// Retain all ancillary metadata chunks instead of stripping them.
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub keep_metadata: bool,

    /// Allow overwriting existing output files in place.
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub overwrite: bool,

    /// Limit the number of worker threads (defaults to logical CPU count).
    #[arg(long, value_parser = clap::value_parser!(NonZeroUsize))]
    pub threads: Option<NonZeroUsize>,

    /// Disable the fancy progress UI and emit plain log lines instead.
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub no_progress: bool,

    /// Preview actions without writing any files.
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Compression quality (only relevant in `compress` mode).
    #[arg(
        long,
        default_value_t = 90u8,
        value_name = "LEVEL",
        value_parser = clap::value_parser!(u8).range(1..=100)
    )]
    pub quality: u8,

    /// Enable exhaustive Zopfli-style DEFLATE even in optimize mode.
    #[arg(long, default_value_t = false, action = ArgAction::SetTrue)]
    pub zopfli: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
pub enum Mode {
    Optimize,
    Compress,
}

/// Derived configuration passed to the pipeline.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub inputs: Vec<PathBuf>,
    pub mode: Mode,
    pub common: CommonOptions,
    pub optimize: OptimizeOptions,
    pub compress: CompressOptions,
}

#[derive(Debug, Clone)]
pub struct CommonOptions {
    pub keep_metadata: bool,
    pub overwrite: bool,
    pub threads: Option<NonZeroUsize>,
    pub progress: ProgressKind,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct OptimizeOptions {
    pub zopfli: bool,
}

#[derive(Debug, Clone)]
pub struct CompressOptions {
    pub quality: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProgressKind {
    Fancy,
    Quiet,
}

impl Cli {
    pub fn build(self) -> Result<AppConfig> {
        if self.inputs.is_empty() {
            bail!("at least one PNG path must be provided");
        }

        let inputs = self
            .inputs
            .into_iter()
            .map(|path| {
                if path.exists() {
                    Ok(path)
                } else {
                    bail!("input path {:?} does not exist", path)
                }
            })
            .collect::<Result<Vec<PathBuf>>>()
            .context("validating input paths")?;

        let common = CommonOptions {
            keep_metadata: self.keep_metadata,
            overwrite: self.overwrite,
            threads: self.threads,
            progress: if self.no_progress {
                ProgressKind::Quiet
            } else {
                ProgressKind::Fancy
            },
            dry_run: self.dry_run,
        };

        let optimize = OptimizeOptions {
            zopfli: self.zopfli,
        };
        let compress = CompressOptions {
            quality: self.quality,
        };

        Ok(AppConfig {
            inputs,
            mode: self.mode,
            common,
            optimize,
            compress,
        })
    }
}

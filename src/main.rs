mod cli;
mod compressor;
mod optimizer;
mod pipeline;
mod ui;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

use crate::cli::{AppConfig, Mode};

fn main() -> Result<()> {
    let parsed = cli::Cli::parse();
    let config = parsed.build()?;

    if let Some(threads) = config.common.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build_global()
            .context("configuring rayon thread pool")?;
    }

    let targets = resolve_inputs(&config)?;
    if targets.is_empty() {
        anyhow::bail!("no PNG files found in the provided inputs");
    }

    let progress = ui::ProgressDispatcher::new(config.common.progress, targets.len());

    match config.mode {
        Mode::Optimize => optimizer::run(optimizer::OptimizeJob {
            inputs: &targets,
            options: &config.optimize,
            common: &config.common,
            progress: &progress,
        }),
        Mode::Compress => compressor::run(compressor::CompressJob {
            inputs: &targets,
            options: &config.compress,
            common: &config.common,
            progress: &progress,
        }),
    }
}

fn resolve_inputs(config: &AppConfig) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in &config.inputs {
        if input.is_dir() {
            collect_from_directory(input, &mut files)?;
        } else if is_png(input) {
            files.push(input.canonicalize().unwrap_or_else(|_| input.clone()));
        }
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    files.retain(|path| seen.insert(path.clone()));

    Ok(files)
}

fn collect_from_directory(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if is_png(path) {
            files.push(path.to_path_buf());
        }
    }

    Ok(())
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches_ignore_ascii_case(ext, "png"))
        .unwrap_or(false)
}

fn matches_ignore_ascii_case(value: &str, needle: &str) -> bool {
    value.eq_ignore_ascii_case(needle)
}

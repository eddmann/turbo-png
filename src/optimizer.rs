use std::fs;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use oxipng::{self, Deflaters, Options};

use crate::cli::{CommonOptions, OptimizeOptions};
use crate::pipeline::{build_strip_policy, derive_output_path, write_atomic};
use crate::ui::{FileOutcome, ProgressDispatcher};

pub struct OptimizeJob<'a> {
    pub inputs: &'a [PathBuf],
    pub options: &'a OptimizeOptions,
    pub common: &'a CommonOptions,
    pub progress: &'a ProgressDispatcher,
}

pub fn run(job: OptimizeJob<'_>) -> Result<()> {
    let mut failures = Vec::new();

    for path in job.inputs {
        job.progress.file_started(path);
        match process_file(path, &job) {
            Ok(outcome) => job.progress.file_finished(path, Some(outcome)),
            Err(err) => {
                job.progress.file_failed(path, &err);
                failures.push(err);
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        let mut message = String::from("one or more files failed during optimization:\n");
        for failure in &failures {
            message.push_str(" • ");
            message.push_str(&failure.to_string());
            message.push('\n');
        }
        bail!(message);
    }
}

fn process_file(path: &Path, job: &OptimizeJob<'_>) -> Result<FileOutcome> {
    let start = Instant::now();

    let output_path =
        derive_output_path(path, "_optimized.png").context("computing optimized output path")?;

    if output_path.exists() && !job.common.overwrite {
        bail!(
            "output file {} already exists (use --overwrite to replace)",
            output_path.display()
        );
    }

    let original_bytes =
        fs::read(path).with_context(|| format!("reading input PNG {}", path.display()))?;
    let original_size = original_bytes.len() as u64;

    let options = configure_options(job.common, job.options)?;

    let optimized_bytes = oxipng::optimize_from_memory(&original_bytes, &options)
        .with_context(|| format!("optimizing {}", path.display()))?;
    let output_size = optimized_bytes.len() as u64;

    if job.common.dry_run {
        return Ok(FileOutcome {
            original_size,
            output_size,
            elapsed: start.elapsed(),
            notes: Some(String::from("dry run")),
        });
    }

    write_atomic(&output_path, &optimized_bytes, job.common.overwrite)
        .with_context(|| format!("writing optimized PNG {}", output_path.display()))?;

    Ok(FileOutcome {
        original_size,
        output_size,
        elapsed: start.elapsed(),
        notes: None,
    })
}

fn configure_options(common: &CommonOptions, options: &OptimizeOptions) -> Result<Options> {
    let mut opts = Options::max_compression();
    opts.fast_evaluation = false;
    opts.strip = build_strip_policy(common)?;
    if options.zopfli {
        opts.deflate = Deflaters::Zopfli {
            iterations: NonZeroU8::new(15).expect("15 is non-zero"),
        };
    }

    Ok(opts)
}

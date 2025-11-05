use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::cli::ProgressKind;

pub struct ProgressDispatcher {
    kind: ProgressKind,
    total: usize,
    processed: RefCell<usize>,
    multi: Option<Arc<MultiProgress>>,
    overall: Option<ProgressBar>,
    current: RefCell<Option<ProgressBar>>,
}

impl ProgressDispatcher {
    pub fn new(kind: ProgressKind, total: usize) -> Self {
        match kind {
            ProgressKind::Quiet => Self {
                kind,
                total,
                processed: RefCell::new(0),
                multi: None,
                overall: None,
                current: RefCell::new(None),
            },
            ProgressKind::Fancy => {
                let multi = Arc::new(MultiProgress::with_draw_target(
                    ProgressDrawTarget::stderr_with_hz(20),
                ));
                let overall = multi.add(ProgressBar::new(total as u64));
                overall.set_style(overall_style());
                overall.set_position(0);
                overall.set_message(format!("0/{} files", total));

                Self {
                    kind,
                    total,
                    processed: RefCell::new(0),
                    multi: Some(multi),
                    overall: Some(overall),
                    current: RefCell::new(None),
                }
            }
        }
    }

    pub fn file_started(&self, path: &Path) {
        match self.kind {
            ProgressKind::Quiet => {}
            ProgressKind::Fancy => {
                let spinner = self
                    .multi
                    .as_ref()
                    .expect("multi exists in fancy mode")
                    .add(ProgressBar::new_spinner());
                spinner.set_style(spinner_style());
                spinner.set_message(format!("processing {}", path.display()));
                spinner.enable_steady_tick(Duration::from_millis(80));
                *self.current.borrow_mut() = Some(spinner);
            }
        }
    }

    pub fn file_finished(&self, path: &Path, outcome: Option<FileOutcome>) {
        match self.kind {
            ProgressKind::Quiet => {}
            ProgressKind::Fancy => {
                if let Some(spinner) = self.current.borrow_mut().take() {
                    let message = match outcome {
                        Some(ref outcome) => format_success(path, outcome),
                        None => format!("✓ {}", path.display()),
                    };
                    spinner.finish_with_message(message);
                }
                self.tick_overall();
            }
        }
    }

    pub fn file_failed(&self, path: &Path, error: &Error) {
        match self.kind {
            ProgressKind::Quiet => {}
            ProgressKind::Fancy => {
                if let Some(spinner) = self.current.borrow_mut().take() {
                    spinner.abandon_with_message(format!("✗ {} ({})", path.display(), error));
                } else if let Some(multi) = &self.multi {
                    let _ = multi.println(format!("✗ {} ({})", path.display(), error));
                } else {
                    eprintln!("✗ {} ({})", path.display(), error);
                }
                self.tick_overall();
            }
        }
    }

    fn tick_overall(&self) {
        if let Some(overall) = &self.overall {
            let mut processed = self.processed.borrow_mut();
            *processed += 1;
            overall.set_position(*processed as u64);
            overall.set_message(format!("{}/{} files", *processed, self.total));

            if *processed == self.total {
                overall.finish_with_message("All files processed");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileOutcome {
    pub original_size: u64,
    pub output_size: u64,
    pub elapsed: Duration,
    pub notes: Option<String>,
}

fn format_success(path: &Path, outcome: &FileOutcome) -> String {
    let original = outcome.original_size;
    let output = outcome.output_size;
    let mut parts = vec![
        format!("{} → {}", format_bytes(original), format_bytes(output)),
        format_savings(original, output),
        format_duration(outcome.elapsed),
    ];

    if let Some(notes) = &outcome.notes {
        parts.push(notes.clone());
    }

    format!("✓ {} ({})", path.display(), parts.join(", "))
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let as_f64 = bytes as f64;
    if as_f64 >= GB {
        format!("{:.2} GiB", as_f64 / GB)
    } else if as_f64 >= MB {
        format!("{:.2} MiB", as_f64 / MB)
    } else if as_f64 >= KB {
        format!("{:.2} KiB", as_f64 / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn format_savings(original: u64, optimized: u64) -> String {
    if original == 0 || optimized >= original {
        let delta = optimized.saturating_sub(original);
        format!("+{}", format_bytes(delta))
    } else {
        let saved = original - optimized;
        let percent = (saved as f64 / original as f64) * 100.0;
        format!("-{} ({:.1}% saved)", format_bytes(saved), percent)
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs_f64() >= 1.0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{} ms", duration.as_millis())
    }
}

fn overall_style() -> ProgressStyle {
    ProgressStyle::with_template("{bar:36.green/black} {pos:>2}/{len} files")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█▉▊▋▌▍▎▏ ")
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_strings(&["⠁", "⠂", "⠄", "⠂"])
}

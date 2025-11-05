# PNG Compressor CLI

High-performance Rust CLI for lossless optimization and visually lossless compression of PNG images—no uploads required.

## Features

- Two modes:
  - `optimize`: maximally lossless transformation (metadata stripping, chunk reordering, DEFLATE refinement).
  - `compress`: palette quantization + quality-controlled tweaks for the smallest visually lossless PNGs.
- Multi-file and directory processing with automatic recursion and deduplication.
- Fancy progress UI (Indicatif) or quiet logging.
- Chunk and metadata retention controls (`--keep-metadata`).
- Atomic writes with `_optimized.png` / `_compressed.png` suffixes.
- Dry-run summaries without touching disk.

## Requirements

- Rust 1.74+ (tested on stable).
- libclang toolchain (for `imagequant`, pulled automatically via Cargo).

## Getting Started

```bash
git clone https://github.com/your-org/png-compressor.git
cd png-compressor
cargo build --release
```

The compiled binary is located at `target/release/png-compressor`.

## Usage

```bash
cargo run --release -- [OPTIONS] <PATH>...
```

### Global Options

| Flag                          | Description                                                            |
| ----------------------------- | ---------------------------------------------------------------------- |
| `--mode <optimize\|compress>` | Select processing pipeline (default: `optimize`).                      |
| `--keep-metadata`             | Preserve all ancillary chunks instead of stripping safe-only metadata. |
| `--overwrite`                 | Replace existing `_optimized.png` / `_compressed.png` outputs.         |
| `--threads <N>`               | Limit Rayon worker threads (defaults to logical CPU count).            |
| `--no-progress`               | Disable the Indicatif UI and emit plain log lines instead.             |
| `--dry-run`                   | Run the full pipeline without writing any files.                       |
| `--zopfli`                    | Force exhaustive Zopfli DEFLATE even in optimize mode.                 |

### Optimize Mode (Lossless)

Delivers the tightest lossless PNG possible. Enable Zopfli for extra squeeze:

```bash
cargo run --release -- --mode optimize --zopfli assets/logo.png
```

Creates `assets/logo_optimized.png` with identical pixels and improved size.

### Compress Mode (Visually Lossless)

Quantizes colors, applies dithering, and recompresses with Zopfli. Tune quality (1–100, default 90):

```bash
cargo run --release -- --mode compress --quality 70 screenshots/*.png
```

Outputs `*_compressed.png`, reporting palette size, savings %, and runtime.

Additional option:

| Flag                | Description                                         |
| ------------------- | --------------------------------------------------- |
| `--quality <LEVEL>` | Palette quantization quality (1–100, default `90`). |

## Metadata Retention Examples

```bash
# Preserve all metadata verbatim
cargo run --release -- --keep-metadata image.png

# Default behavior keeps display-critical metadata (ICC profiles, color space,
# and APNG animation chunks) consistent with oxipng’s `--strip safe` mode.
```

## Progress UI

By default the tool displays:

- Per-file spinner with live status.
- Overall progress bar (`processed/total`).
- Summary line containing original vs optimized size, savings %, elapsed time, and optional notes.

Disable with `--no-progress` for CI logs.

## Roadmap / Ideas

- PNG validation report prior to transform.
- Optional WebP/AVIF fallback pipeline.
- Config file for project-specific defaults.
- Benchmark suite comparing external tools (e.g., pngquant, ImageOptim).

## Contributing

1. Fork and clone.
2. `cargo fmt && cargo check` before submitting PRs.
3. Add tests or sample fixtures when touching compression logic.

## License

Dual-licensed for GPL/commercial usage via the `imagequant` dependency. Consult upstream licensing if you intend to redistribute binaries. All additional project code is released under MIT unless otherwise noted.

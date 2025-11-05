# TurboPNG

![TurboPNG](docs/heading_compressed.png)

TurboPNG is a high-performance Rust CLI for lossless optimization and size-focused PNG compression.

> Built after I got tired of uploading ChatGPT-generated PNGs to random compression websites, this CLI handles lossless and size-focused compression for those bloated outputs.

## Features

- Two modes:
  - `optimize`: maximally lossless transformation (metadata stripping, chunk reordering, DEFLATE refinement).
  - `compress`: aggressive palette squeezing (≤32 colors by default) with filterless Zopfli for graphics; `--quality 98` unlocks a photo-friendly palette + adaptive filters.
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
git clone https://github.com/eddmann/turbo-png.git
cd turbo-png
cargo build --release
```

The compiled binary is located at `target/release/turbo-png`.

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

### Compress Mode (Graphics-Focused)

Aims for the smallest PNGs on flat artwork by quantizing to a tight palette (typically ≤32 colors), disabling PNG filtering, and recompressing with Zopfli. Tune quality (1–100, default 90):

```bash
cargo run --release -- --mode compress --quality 70 screenshots/*.png
```

Outputs `*_compressed.png`, reporting palette size, savings %, and runtime.

> Quality controls the palette cap (roughly 12–48 colors) and dithering strength. `--quality 98` activates a photo-friendly preset (≈96 colors + adaptive filters) for smoother gradients and photographic content.

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

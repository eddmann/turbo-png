use std::fs;
use std::io::Cursor;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use imagequant::{self, RGBA};
use oxipng::{self, Deflaters, Options, RowFilter, StripChunks, indexset};
use png::chunk::ChunkType;
use png::{self, AdaptiveFilterType, BitDepth, ColorType, Compression, Encoder, FilterType};

use crate::cli::{CommonOptions, CompressOptions};
use crate::pipeline::{build_strip_policy, derive_output_path, strip_policy_allows, write_atomic};
use crate::ui::{FileOutcome, ProgressDispatcher};

pub struct CompressJob<'a> {
    pub inputs: &'a [PathBuf],
    pub options: &'a CompressOptions,
    pub common: &'a CommonOptions,
    pub progress: &'a ProgressDispatcher,
}

pub fn run(job: CompressJob<'_>) -> Result<()> {
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
        let mut message = String::from("one or more files failed during compression:\n");
        for failure in &failures {
            message.push_str(" • ");
            message.push_str(&failure.to_string());
            message.push('\n');
        }
        bail!(message);
    }
}

fn process_file(path: &Path, job: &CompressJob<'_>) -> Result<FileOutcome> {
    let start = Instant::now();

    let output_path =
        derive_output_path(path, "_compressed.png").context("computing compressed output path")?;

    if output_path.exists() && !job.common.overwrite {
        bail!(
            "output file {} already exists (use --overwrite to replace)",
            output_path.display()
        );
    }

    let original_bytes =
        fs::read(path).with_context(|| format!("reading input PNG {}", path.display()))?;
    let original_size = original_bytes.len() as u64;

    let strip_policy = build_strip_policy(job.common)?;
    let preserved = extract_preserved_chunks(&original_bytes, &strip_policy)
        .context("extracting metadata chunks")?;
    let decoded = decode_rgba(&original_bytes).context("decoding PNG")?;
    let quantized =
        quantize_image(&decoded, job.options.quality).context("quantizing image to palette")?;
    let palette_note = format!("{} colors", quantized.palette.len());
    let indexed_png = encode_indexed_png(&quantized, &decoded, &preserved, job.options.quality)
        .context("encoding indexed PNG")?;

    let mut options = configure_options(job.common, job.options);
    options.strip = strip_policy.clone();

    let optimized_bytes = oxipng::optimize_from_memory(&indexed_png, &options)
        .with_context(|| format!("optimizing {}", path.display()))?;
    let output_size = optimized_bytes.len() as u64;

    if job.common.dry_run {
        return Ok(FileOutcome {
            original_size,
            output_size,
            elapsed: start.elapsed(),
            notes: Some(format!("dry run, {palette_note}")),
        });
    }

    write_atomic(&output_path, &optimized_bytes, job.common.overwrite)
        .with_context(|| format!("writing compressed PNG {}", output_path.display()))?;

    Ok(FileOutcome {
        original_size,
        output_size,
        elapsed: start.elapsed(),
        notes: Some(palette_note),
    })
}

struct DecodedImage {
    width: u32,
    height: u32,
    pixels: Vec<RGBA>,
}

struct QuantizedImage {
    palette: Vec<RGBA>,
    indices: Vec<u8>,
}

struct PreservedChunks {
    before_idat: Vec<PngChunk>,
    after_idat: Vec<PngChunk>,
}

struct PngChunk {
    name: [u8; 4],
    data: Vec<u8>,
}

fn decode_rgba(bytes: &[u8]) -> Result<DecodedImage> {
    let cursor = Cursor::new(bytes);
    let mut decoder = png::Decoder::new(cursor);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().context("reading PNG info")?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .context("reading PNG image data")?;

    if info.bit_depth != BitDepth::Eight {
        bail!("expected 8-bit output after decoding");
    }

    let pixel_count = info.width as usize * info.height as usize;
    let mut pixels = Vec::with_capacity(pixel_count);

    match info.color_type {
        ColorType::Rgba => {
            let expected_len = pixel_count * 4;
            let data = &buffer[..expected_len];
            for chunk in data.chunks_exact(4) {
                pixels.push(RGBA::new(chunk[0], chunk[1], chunk[2], chunk[3]));
            }
        }
        ColorType::Rgb => {
            let expected_len = pixel_count * 3;
            let data = &buffer[..expected_len];
            for chunk in data.chunks_exact(3) {
                pixels.push(RGBA::new(chunk[0], chunk[1], chunk[2], 255));
            }
        }
        other => {
            bail!("unsupported color type after decoding: {:?}", other);
        }
    }

    Ok(DecodedImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

fn quantize_image(image: &DecodedImage, quality: u8) -> Result<QuantizedImage> {
    let mut attr = imagequant::new();
    let quality = quality.clamp(1, 100);
    let (quality_min, quality_target) = select_quality_window(quality);
    attr.set_quality(quality_min, quality_target)?;
    attr.set_max_colors(select_palette_cap(quality))?;
    attr.set_speed(i32::from(select_speed(quality)))?;

    let mut liq_image = attr.new_image_borrowed(
        &image.pixels,
        image.width as usize,
        image.height as usize,
        0.0,
    )?;
    let mut result = attr.quantize(&mut liq_image)?;
    result.set_dithering_level(select_dithering(quality))?;
    let (palette, indices) = result.remapped(&mut liq_image)?;

    Ok(QuantizedImage { palette, indices })
}

fn encode_indexed_png(
    quantized: &QuantizedImage,
    decoded: &DecodedImage,
    preserved: &PreservedChunks,
    quality: u8,
) -> Result<Vec<u8>> {
    if quantized.palette.is_empty() {
        bail!("quantizer returned an empty palette");
    }
    if quantized.palette.len() > 256 {
        bail!("quantizer produced more than 256 colors");
    }

    let expected = decoded.width as usize * decoded.height as usize;
    if quantized.indices.len() != expected {
        bail!("quantized pixel buffer has unexpected length");
    }

    let mut palette_bytes = Vec::with_capacity(quantized.palette.len() * 3);
    let mut alpha_bytes = Vec::with_capacity(quantized.palette.len());
    for color in &quantized.palette {
        palette_bytes.extend([color.r, color.g, color.b]);
        alpha_bytes.push(color.a);
    }

    while matches!(alpha_bytes.last(), Some(&255)) {
        alpha_bytes.pop();
    }

    let mut output = Vec::new();
    {
        let mut encoder = Encoder::new(&mut output, decoded.width, decoded.height);
        encoder.set_color(ColorType::Indexed);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_palette(palette_bytes);
        if !alpha_bytes.is_empty() {
            encoder.set_trns(alpha_bytes);
        }
        encoder.set_compression(Compression::Best);
        if is_photo_quality(quality) {
            encoder.set_filter(FilterType::Paeth);
            encoder.set_adaptive_filter(AdaptiveFilterType::Adaptive);
        } else {
            encoder.set_filter(FilterType::NoFilter);
            encoder.set_adaptive_filter(AdaptiveFilterType::NonAdaptive);
        }

        let mut writer = encoder.write_header()?;
        for chunk in &preserved.before_idat {
            writer.write_chunk(ChunkType(chunk.name), &chunk.data)?;
        }
        writer.write_image_data(&quantized.indices)?;
        for chunk in &preserved.after_idat {
            writer.write_chunk(ChunkType(chunk.name), &chunk.data)?;
        }
        writer.finish()?;
    }

    Ok(output)
}

fn extract_preserved_chunks(data: &[u8], policy: &StripChunks) -> Result<PreservedChunks> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if data.len() < SIGNATURE.len() || data[..8] != SIGNATURE {
        bail!("file is not a valid PNG");
    }

    let mut index = SIGNATURE.len();
    let mut before_idat = Vec::new();
    let mut after_idat = Vec::new();
    let mut seen_idat = false;

    while index + 12 <= data.len() {
        let length = u32::from_be_bytes(data[index..index + 4].try_into().unwrap()) as usize;
        index += 4;
        if index + 4 > data.len() {
            bail!("truncated PNG chunk header");
        }
        let mut name = [0u8; 4];
        name.copy_from_slice(&data[index..index + 4]);
        index += 4;

        if index + length + 4 > data.len() {
            bail!("truncated PNG chunk data");
        }
        let chunk_data = data[index..index + length].to_vec();
        index += length + 4; // skip data and CRC

        match &name {
            b"IDAT" => {
                seen_idat = true;
                continue;
            }
            b"IHDR" | b"IEND" | b"PLTE" | b"tRNS" => continue,
            _ => {}
        }

        if strip_policy_allows(policy, &name) {
            let chunk = PngChunk {
                name,
                data: chunk_data,
            };
            if seen_idat {
                after_idat.push(chunk);
            } else {
                before_idat.push(chunk);
            }
        }
    }

    Ok(PreservedChunks {
        before_idat,
        after_idat,
    })
}

fn configure_options(_common: &CommonOptions, opts: &CompressOptions) -> Options {
    let mut options = Options::max_compression();
    options.fast_evaluation = false;
    if is_photo_quality(opts.quality) {
        options.filter = indexset! {
            RowFilter::None,
            RowFilter::Sub,
            RowFilter::Up,
            RowFilter::Average,
            RowFilter::Paeth
        };
    } else {
        options.filter = indexset! { RowFilter::None };
    }
    options.bit_depth_reduction = false;
    options.color_type_reduction = false;
    options.palette_reduction = false;
    options.grayscale_reduction = false;
    let iterations = select_zopfli_iterations(opts.quality);
    options.deflate = Deflaters::Zopfli {
        iterations: NonZeroU8::new(iterations).expect("iterations > 0"),
    };
    options
}

fn select_quality_window(quality: u8) -> (u8, u8) {
    match quality {
        98..=100 => (85, 99),
        95..=97 => (80, 96),
        85..=94 => (70, 92),
        70..=84 => (60, 88),
        55..=69 => (45, 82),
        40..=54 => (35, 76),
        _ => (25, 68),
    }
}

fn select_palette_cap(quality: u8) -> u32 {
    match quality {
        98..=100 => 96,
        95..=97 => 48,
        85..=94 => 32,
        70..=84 => 24,
        55..=69 => 20,
        40..=54 => 16,
        _ => 12,
    }
}

fn is_photo_quality(quality: u8) -> bool {
    quality >= 98
}

fn select_speed(quality: u8) -> u8 {
    match quality {
        90..=100 => 1,
        75..=89 => 3,
        50..=74 => 5,
        30..=49 => 7,
        _ => 9,
    }
}

fn select_dithering(quality: u8) -> f32 {
    match quality {
        90..=100 => 1.0,
        75..=89 => 0.8,
        50..=74 => 0.6,
        30..=49 => 0.4,
        _ => 0.3,
    }
}

fn select_zopfli_iterations(quality: u8) -> u8 {
    match quality {
        95..=100 => 25,
        80..=94 => 20,
        60..=79 => 15,
        40..=59 => 12,
        _ => 10,
    }
}

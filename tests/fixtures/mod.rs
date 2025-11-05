#![allow(dead_code)]

use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use assert_fs::TempDir;
use assert_fs::fixture::PathChild;
use png::chunk;
use png::{BitDepth, ColorType, Compression, Decoder, Encoder, FilterType, Transformations};

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn write_fixture(dir: &TempDir, name: &str) -> PathBuf {
    let child = dir.child(name);
    let path = child.path().to_path_buf();
    let file = File::create(&path).expect("failed to create fixture file");
    let mut writer = BufWriter::new(file);

    let mut encoder = Encoder::new(&mut writer, 1, 1);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut png_writer = encoder.write_header().expect("failed to write PNG header");

    let phys_chunk: [u8; 9] = [0, 0, 0x03, 0xE8, 0, 0, 0x03, 0xE8, 1];
    png_writer
        .write_chunk(chunk::pHYs, &phys_chunk)
        .expect("failed to write pHYs chunk");

    let text_chunk = b"Comment\0licensed";
    png_writer
        .write_chunk(chunk::tEXt, text_chunk)
        .expect("failed to write tEXt chunk");

    png_writer
        .write_image_data(&[255, 0, 0, 255])
        .expect("failed to write image data");
    png_writer.finish().expect("failed to finish PNG");

    path
}

pub fn write_unoptimized_rgba(dir: &TempDir, name: &str, width: u32, height: u32) -> PathBuf {
    let pixels = noisy_pixels(width, height);
    write_rgba_png(dir, name, width, height, &pixels, Compression::Fast)
}

pub fn write_palette_source(dir: &TempDir, name: &str) -> PathBuf {
    let width = 16;
    let height = 16;
    let palette = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];
    let mut pixels = Vec::with_capacity((width * height) as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let idx = ((x / 4 + y / 4) % palette.len() as u32) as usize;
            pixels.extend_from_slice(&palette[idx]);
        }
    }
    write_rgba_png(dir, name, width, height, &pixels, Compression::Fast)
}

pub fn derived_output_path(input: &Path, suffix: &str) -> PathBuf {
    let parent = input.parent().unwrap();
    let stem = input.file_stem().unwrap().to_string_lossy();
    parent.join(format!("{}{}", stem, suffix))
}

pub fn chunk_names(path: &Path) -> Vec<[u8; 4]> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let data = std::fs::read(path).expect("failed to read PNG");
    assert!(data.starts_with(&SIGNATURE), "fixture is not a PNG");

    let mut index = SIGNATURE.len();
    let mut names = Vec::new();
    while index + 8 <= data.len() {
        let length = u32::from_be_bytes(data[index..index + 4].try_into().unwrap()) as usize;
        index += 4;
        let name: [u8; 4] = data[index..index + 4].try_into().unwrap();
        index += 4;
        names.push(name);
        index += length + 4; // skip data + CRC
        if name == *b"IEND" {
            break;
        }
    }

    names
}

pub fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .unwrap_or_else(|err| panic!("failed to read metadata for {}: {err}", path.display()))
        .len()
}

pub fn decode_rgba(path: &Path) -> DecodedImage {
    let file =
        File::open(path).unwrap_or_else(|err| panic!("failed to open {}: {err}", path.display()));
    let mut decoder = Decoder::new(file);
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("failed to read PNG info");
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .expect("failed to read PNG frame");
    let raw = &buffer[..info.buffer_size()];
    let data = match info.color_type {
        ColorType::Rgba => raw.to_vec(),
        ColorType::Rgb => raw
            .chunks_exact(3)
            .flat_map(|chunk| [chunk[0], chunk[1], chunk[2], 255])
            .collect(),
        ColorType::Grayscale => raw.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        ColorType::GrayscaleAlpha => raw
            .chunks_exact(2)
            .flat_map(|chunk| [chunk[0], chunk[0], chunk[0], chunk[1]])
            .collect(),
        other => panic!(
            "unsupported color type {other:?} when decoding {}",
            path.display()
        ),
    };

    DecodedImage {
        width: info.width,
        height: info.height,
        data,
    }
}

pub fn encoded_color_type(path: &Path) -> (ColorType, Option<usize>) {
    let file =
        File::open(path).unwrap_or_else(|err| panic!("failed to open {}: {err}", path.display()));
    let mut decoder = Decoder::new(file);
    decoder.set_transformations(Transformations::IDENTITY);
    let reader = decoder.read_info().expect("failed to read PNG info");
    let info = reader.info();
    let palette_len = info.palette.as_ref().map(|p| p.len() / 3);
    (info.color_type, palette_len)
}

pub fn unique_color_count(image: &DecodedImage) -> usize {
    image
        .data
        .chunks_exact(4)
        .map(|chunk| <[u8; 4]>::try_from(chunk).expect("pixel chunk"))
        .collect::<HashSet<_>>()
        .len()
}

pub fn max_abs_channel_difference(a: &DecodedImage, b: &DecodedImage) -> u8 {
    assert_eq!(a.width, b.width);
    assert_eq!(a.height, b.height);
    assert_eq!(a.data.len(), b.data.len());
    a.data
        .iter()
        .zip(&b.data)
        .fold(0u8, |acc, (left, right)| acc.max(left.abs_diff(*right)))
}

fn write_rgba_png(
    dir: &TempDir,
    name: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
    compression: Compression,
) -> PathBuf {
    let child = dir.child(name);
    let path = child.path().to_path_buf();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create fixture directory");
    }
    let file = File::create(&path).expect("failed to create PNG");
    let mut writer = BufWriter::new(file);

    let mut encoder = Encoder::new(&mut writer, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(compression);
    encoder.set_filter(FilterType::Up);
    let mut png_writer = encoder.write_header().expect("failed to write PNG header");
    png_writer
        .write_image_data(pixels)
        .expect("failed to write PNG pixels");
    png_writer.finish().expect("failed to finalize PNG");

    path
}

fn noisy_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let base = ((x * 37 + y * 19) % 256) as u8;
            pixels.extend_from_slice(&[base, base.wrapping_add(53), base.wrapping_add(101), 255]);
        }
    }
    pixels
}

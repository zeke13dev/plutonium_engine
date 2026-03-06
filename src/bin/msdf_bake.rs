use anyhow::{anyhow, Context, Result};
use plutonium_engine::text::{
    Bounds, MsdfAtlasInfo, MsdfFontMetadata, MsdfGlyphRecord, MsdfKerningRecord, MsdfMetrics,
    TextRenderer,
};
use rusttype::{Font, Scale};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ASCII_START: u8 = 32;
const ASCII_END: u8 = 126;

#[derive(Debug)]
struct BakeArgs {
    font_path: PathBuf,
    out_dir: PathBuf,
    name: String,
    logical_font_size: f32,
    generation_scale: f32,
    padding: u32,
    px_range: f32,
}

fn print_help() {
    eprintln!(
        "\
msdf_bake: offline ASCII MSDF atlas generator

Usage:
  cargo run --bin msdf_bake -- --font <path.ttf> --out-dir <dir> [options]

Required:
  --font <path>            Path to input TTF/OTF font file
  --out-dir <dir>          Directory for output assets

Options:
  --name <base>            Output base name (default: font file stem)
  --font-size <px>         Logical font size stored in metadata (default: 32)
  --gen-scale <multiplier> Internal generation scale multiplier (default: 4.0)
  --padding <px>           Glyph tile padding in generated atlas (default: 6)
  --px-range <px>          Distance range used during bake (default: 8.0)
  --help                   Show this help

Outputs:
  <out-dir>/<name>.msdf.png
  <out-dir>/<name>.msdf.json

Example:
  cargo run --bin msdf_bake -- \
    --font examples/media/roboto.ttf \
    --out-dir examples/media \
    --name roboto
"
    );
}

fn parse_next<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T> {
    let raw = iter
        .next()
        .ok_or_else(|| anyhow!("missing value for {}", flag))?;
    raw.parse::<T>()
        .map_err(|_| anyhow!("invalid value '{}' for {}", raw, flag))
}

fn parse_args() -> Result<BakeArgs> {
    let mut args_iter = env::args().skip(1);

    if env::args().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Err(anyhow!("help"));
    }

    let mut font_path: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut logical_font_size = 32.0f32;
    let mut generation_scale = 4.0f32;
    let mut padding = 10u32;
    let mut px_range = 8.0f32;

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--font" => {
                font_path = Some(PathBuf::from(parse_next::<String>(
                    &mut args_iter,
                    "--font",
                )?))
            }
            "--out-dir" => {
                out_dir = Some(PathBuf::from(parse_next::<String>(
                    &mut args_iter,
                    "--out-dir",
                )?))
            }
            "--name" => name = Some(parse_next::<String>(&mut args_iter, "--name")?),
            "--font-size" => logical_font_size = parse_next::<f32>(&mut args_iter, "--font-size")?,
            "--gen-scale" => generation_scale = parse_next::<f32>(&mut args_iter, "--gen-scale")?,
            "--padding" => padding = parse_next::<u32>(&mut args_iter, "--padding")?,
            "--px-range" => px_range = parse_next::<f32>(&mut args_iter, "--px-range")?,
            _ => return Err(anyhow!("unknown argument '{}'", arg)),
        }
    }

    let font_path = font_path.ok_or_else(|| anyhow!("--font is required"))?;
    let out_dir = out_dir.ok_or_else(|| anyhow!("--out-dir is required"))?;
    let name = match name {
        Some(v) => v,
        None => font_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("unable to infer output name from font path"))?
            .to_string(),
    };

    if logical_font_size <= 0.0 {
        return Err(anyhow!("--font-size must be > 0"));
    }
    if generation_scale < 1.0 {
        return Err(anyhow!("--gen-scale must be >= 1.0"));
    }
    if px_range <= 0.0 {
        return Err(anyhow!("--px-range must be > 0"));
    }

    Ok(BakeArgs {
        font_path,
        out_dir,
        name,
        logical_font_size,
        generation_scale,
        padding,
        px_range,
    })
}

fn build_metadata(
    font: &Font<'_>,
    _font_data: &[u8],
    logical_font_size: f32,
    generation_scale: f32,
    padding: u32,
    px_range: f32,
) -> Result<(Vec<u8>, u32, u32, MsdfFontMetadata)> {
    let generation_font_size = (logical_font_size * generation_scale).max(logical_font_size);
    let scale = Scale::uniform(generation_font_size);

    let (atlas_width, atlas_height, char_dimensions, max_tile_width, max_tile_height) =
        TextRenderer::calculate_atlas_size(font, scale, padding);
    let (texture_data, char_map) = TextRenderer::render_msdf_glyphs_to_atlas(
        font,
        scale,
        (atlas_width, atlas_height),
        &char_dimensions,
        padding,
        px_range,
    )
    .ok_or_else(|| anyhow!("failed to rasterize MSDF atlas; atlas sizing was insufficient"))?;

    let cols = (atlas_width / max_tile_width.max(1)).max(1);
    let denom = generation_font_size.max(1.0);
    // Keep advance metrics in the same rusttype space used by raster text.
    let shaping_advances: Option<HashMap<char, f32>> = None;
    // Keep MSDF kerning in lockstep with raster path semantics (rusttype pair_kerning).
    // This renderer does not apply per-glyph x_offset, so shaping-derived pair
    // offsets can produce directional spacing drift (e.g. `r/t` vs `j/u`).
    let mut kerning = Vec::new();
    for left_u in ASCII_START..=ASCII_END {
        let left = left_u as char;
        let left_id = font.glyph(left).id();
        for right_u in ASCII_START..=ASCII_END {
            let right = right_u as char;
            let right_id = font.glyph(right).id();
            let k = font.pair_kerning(scale, left_id, right_id) / denom;
            if k.abs() > f32::EPSILON {
                kerning.push(MsdfKerningRecord {
                    left_unicode: left as u32,
                    right_unicode: right as u32,
                    advance: k,
                });
            }
        }
    }

    // rusttype normalises glyph metrics by (hhea ascent − descent) instead of
    // units-per-em.  When these differ (common: Roboto has height 2400, upem
    // 2048) every rusttype-derived em value is off by the ratio height/upem.
    // Compute that ratio so we can correct plane bounds, padding, and metrics.
    let em_correction =
        TextRenderer::compute_rusttype_em_correction(&shaping_advances, &char_map, denom);

    // Keep metadata in the same unit space as rusttype atlas sampling geometry.
    // Shaping data is in upem-space; convert to rusttype-space when available.
    let shaping_advances = shaping_advances.map(|mut advs| {
        for value in advs.values_mut() {
            *value /= em_correction.max(1e-6);
        }
        advs
    });
    let mut glyphs = Vec::new();
    for code in ASCII_START..=ASCII_END {
        let ch = code as char;
        let Some(info) = char_map.get(&ch) else {
            continue;
        };

        let col = (info.tile_index as u32) % cols;
        let row = (info.tile_index as u32) / cols;
        let atlas_left = col as f32 * max_tile_width as f32;
        let atlas_top = row as f32 * max_tile_height as f32;

        let positioned = font
            .glyph(ch)
            .scaled(scale)
            .positioned(rusttype::point(0.0, info.bearing.1));
        let plane_bounds = positioned
            .pixel_bounding_box()
            .map(|bb| {
                TextRenderer::msdf_plane_bounds_from_pixel_bounds(bb, info.bearing.1, denom, 1.0)
            })
            .unwrap_or(Bounds {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            });

        // Use shaping-derived advance if available, otherwise fallback to rusttype.
        let advance = shaping_advances
            .as_ref()
            .and_then(|advs| advs.get(&ch).copied())
            .unwrap_or(info.advance_width / denom);

        glyphs.push(MsdfGlyphRecord {
            unicode: ch as u32,
            advance,
            plane_bounds,
            atlas_bounds: Bounds {
                // atlas_bounds must include baked padding so screen quads can sample the
                // full distance field (not just the tight shape region).
                left: atlas_left,
                top: atlas_top,
                right: atlas_left + info.size.0 as f32,
                bottom: atlas_top + info.size.1 as f32,
            },
        });
    }

    let v_metrics = font.v_metrics(scale);
    let metadata = MsdfFontMetadata {
        atlas: MsdfAtlasInfo {
            width: atlas_width as f32,
            height: atlas_height as f32,
            kind: "msdf".to_string(),
        },
        metrics: MsdfMetrics {
            font_size: logical_font_size.max(1.0),
            ascender: v_metrics.ascent / denom,
            descender: v_metrics.descent / denom,
            line_height: (v_metrics.ascent - v_metrics.descent + v_metrics.line_gap).abs() / denom,
            padding_em: padding as f32 / denom,
            px_range,
        },
        glyphs,
        kerning,
    };

    Ok((texture_data, atlas_width, atlas_height, metadata))
}

fn write_outputs(
    out_dir: &Path,
    name: &str,
    texture_data: &[u8],
    atlas_width: u32,
    atlas_height: u32,
    metadata: &MsdfFontMetadata,
) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(out_dir).with_context(|| {
        format!(
            "failed to create output directory '{}'",
            out_dir.to_string_lossy()
        )
    })?;

    let png_path = out_dir.join(format!("{}.msdf.png", name));
    let json_path = out_dir.join(format!("{}.msdf.json", name));

    image::save_buffer_with_format(
        &png_path,
        texture_data,
        atlas_width,
        atlas_height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .with_context(|| format!("failed to write png '{}'", png_path.to_string_lossy()))?;

    let json = serde_json::to_string_pretty(metadata).context("failed to serialize metadata")?;
    fs::write(&json_path, json)
        .with_context(|| format!("failed to write json '{}'", json_path.to_string_lossy()))?;

    Ok((png_path, json_path))
}

fn run() -> Result<()> {
    let args = parse_args()?;
    let font_data = fs::read(&args.font_path).with_context(|| {
        format!(
            "failed to read font file '{}'",
            args.font_path.to_string_lossy()
        )
    })?;
    let font = Font::try_from_bytes(&font_data).ok_or_else(|| {
        anyhow!(
            "invalid font data in '{}'",
            args.font_path.to_string_lossy()
        )
    })?;

    let (texture_data, atlas_width, atlas_height, metadata) = build_metadata(
        &font,
        &font_data,
        args.logical_font_size,
        args.generation_scale,
        args.padding,
        args.px_range,
    )?;

    let (png_path, json_path) = write_outputs(
        &args.out_dir,
        &args.name,
        &texture_data,
        atlas_width,
        atlas_height,
        &metadata,
    )?;

    println!("MSDF bake complete.");
    println!("  atlas: {}", png_path.to_string_lossy());
    println!("  meta : {}", json_path.to_string_lossy());
    println!("  atlas size: {}x{}", atlas_width, atlas_height);
    println!("  glyph count: {}", metadata.glyphs.len());
    println!(
        "  settings: font-size={} gen-scale={} padding={} px-range={} charset=ASCII(32..=126)",
        args.logical_font_size, args.generation_scale, args.padding, args.px_range
    );
    println!();
    println!("Load in engine with:");
    println!(
        "  engine.load_msdf_font(\"{}\", \"{}\", \"{}\");",
        png_path.to_string_lossy(),
        json_path.to_string_lossy(),
        args.name
    );

    Ok(())
}

fn main() -> Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(err) => {
            if err.to_string() == "help" {
                return Ok(());
            }
            Err(err)
        }
    }
}

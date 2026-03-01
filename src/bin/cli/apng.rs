//! APNG (Animated PNG) エンコードモジュール。
//!
//! フレームループ:
//!   1. elapsed = frame_idx / fps で経過時間を計算
//!   2. render_frame でフレームをレンダリング
//!   3. tiny-skia の premultiplied alpha → straight alpha に変換
//!   4. png クレートの APNG API でフレームを書き出し

use std::io::BufWriter;
use std::path::Path;

use svg::SvgDocument;

use crate::render;

/// APNG ファイルを生成する。
///
/// - `fps`: フレームレート (例: 30)
/// - `duration`: アニメーション全体の秒数 (例: 3.0)
pub fn encode_apng(
    doc: &SvgDocument,
    width: u32,
    height: u32,
    fps: u32,
    duration: f64,
    output: &Path,
) {
    let total_frames = (fps as f64 * duration).ceil() as u32;

    let file = std::fs::File::create(output).expect("Failed to create output file");
    let w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(total_frames, 0)
        .expect("Failed to set animated");
    // フレーム間隔: 1/fps 秒 = 1 / fps
    encoder
        .set_frame_delay(1, fps as u16)
        .expect("Failed to set frame delay");

    let mut writer = encoder.write_header().expect("Failed to write PNG header");

    for frame_idx in 0..total_frames {
        let elapsed = frame_idx as f64 / fps as f64;
        let pixmap = render::render_frame(doc, width, height, elapsed);

        // tiny-skia は premultiplied alpha を使用するため straight alpha に変換する
        let straight = unpremultiply(pixmap.data());

        if frame_idx > 0 {
            writer
                .set_frame_delay(1, fps as u16)
                .expect("Failed to set frame delay");
        }
        writer
            .write_image_data(&straight)
            .expect("Failed to write frame");
    }

    writer.finish().expect("Failed to finish PNG");

    eprintln!(
        "Wrote {} frames ({:.1}s @ {} fps) to {}",
        total_frames,
        duration,
        fps,
        output.display()
    );
}

/// Premultiplied RGBA → Straight RGBA に変換する。
///
/// premultiplied: R' = R * A / 255, G' = G * A / 255, B' = B * A / 255
/// straight:      R = R' * 255 / A, G = G' * 255 / A, B = B' * 255 / A
///
/// A=0 のピクセルは (0,0,0,0) として出力する。
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        let a = chunk[3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else if a == 255 {
            out.extend_from_slice(chunk);
        } else {
            let r = ((chunk[0] as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            let g = ((chunk[1] as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            let b = ((chunk[2] as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    out
}

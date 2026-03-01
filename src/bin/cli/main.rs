//! svg-cli: SVG → PNG / APNG 変換ツール。
//!
//! 使用例:
//!   # 静止画 PNG 出力
//!   svg-cli -i input.svg -o output.png
//!
//!   # APNG アニメーション出力
//!   svg-cli -i input.svg -o output.apng --fps 30 --duration 3
//!
//!   # サイズ指定
//!   svg-cli -i input.svg -o output.png --width 800 --height 600

mod animation;
mod apng;
mod render;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "svg-cli", about = "SVG to PNG/APNG converter")]
struct Args {
    /// 入力 SVG ファイルパス
    #[arg(short = 'i', long)]
    input: PathBuf,

    /// 出力ファイルパス (.png or .apng)
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// APNG のフレームレート
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// APNG のアニメーション秒数
    #[arg(long, default_value_t = 3.0)]
    duration: f64,

    /// 出力画像の幅 (省略時は viewBox から自動計算)
    #[arg(long)]
    width: Option<u32>,

    /// 出力画像の高さ (省略時は viewBox から自動計算)
    #[arg(long)]
    height: Option<u32>,

    /// 静止画 PNG 出力時のアニメーション時刻 (秒)
    #[arg(long, default_value_t = 0.0)]
    time: f64,
}

fn main() {
    let args = Args::parse();

    let data = std::fs::read(&args.input).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", args.input.display(), e);
        std::process::exit(1);
    });

    let doc = svg::parse(&data).unwrap_or_else(|e| {
        eprintln!("Failed to parse SVG: {}", e);
        std::process::exit(1);
    });

    let (width, height) = resolve_size(&doc, args.width, args.height);

    let ext = args
        .output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "apng" => {
            apng::encode_apng(&doc, width, height, args.fps, args.duration, &args.output);
        }
        _ => {
            // 静止画 PNG (指定時刻のスナップショット)
            let pixmap = render::render_frame(&doc, width, height, args.time);
            pixmap.save_png(&args.output).unwrap_or_else(|e| {
                eprintln!("Failed to save PNG: {}", e);
                std::process::exit(1);
            });
            eprintln!("Wrote {}x{} PNG to {}", width, height, args.output.display());
        }
    }
}

/// 出力サイズを決定する。
///
/// - width と height が両方指定: そのまま使用
/// - 片方のみ指定: viewBox のアスペクト比で他方を計算
/// - 両方省略: viewBox のサイズを使用
fn resolve_size(doc: &svg::SvgDocument, w: Option<u32>, h: Option<u32>) -> (u32, u32) {
    match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let scale = w as f32 / doc.view_box.width;
            (w, (doc.view_box.height * scale) as u32)
        }
        (None, Some(h)) => {
            let scale = h as f32 / doc.view_box.height;
            ((doc.view_box.width * scale) as u32, h)
        }
        (None, None) => (doc.view_box.width as u32, doc.view_box.height as u32),
    }
}

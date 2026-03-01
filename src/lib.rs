pub mod types;
mod parse;
mod xml;
mod svg;
mod path;
mod smil;

pub use types::*;

/// SVG バイト列をパースして SvgDocument を返す。
pub fn parse(data: &[u8]) -> Result<SvgDocument, SvgError> {
    parse::parse(data)
}

/// cubic bezier パスを折れ線にフラッテニングする。
/// tolerance はピクセル単位の近似誤差許容値 (例: 0.25)。
/// サブパスごとにフラッテニングした結果を返す。
pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<(f32, f32)>> {
    path::flatten::flatten(segments, tolerance)
}

/// ストロークパスをフィルパス (アウトライン) に変換する。
pub fn stroke_to_fill(segments: &[PathSegment], style: &StrokeStyle) -> Vec<PathSegment> {
    path::stroke::stroke_to_fill(segments, style)
}

/// SMIL アニメーションの現在値を計算する。
/// elapsed_secs はアニメーション開始からの経過秒数。
/// アニメーションが begin 前なら None を返す。
pub fn evaluate(animation: &SmilAnimation, elapsed_secs: f64) -> Option<SmilValue> {
    smil::evaluator::evaluate(animation, elapsed_secs)
}

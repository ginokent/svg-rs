//! レンダリングエンジン。
//!
//! svg クレートのシーンツリーを tiny-skia で描画する。
//! パイプライン:
//!   1. viewBox → 出力サイズへのスケーリング変換を計算
//!   2. シーンツリーを再帰的に走査
//!   3. SvgGroup: transform を合成して子要素を描画
//!   4. SvgPath: PathSegment → tiny_skia::PathBuilder に変換し fill/stroke を描画

use svg::{
    Affine2D, FillRule, LineCap, LineJoin, PathSegment, SvgDocument, SvgGroup, SvgNode, SvgPath,
};
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

use crate::animation::AnimationState;

/// 指定時刻でのフレームをレンダリングする。
pub fn render_frame(
    doc: &SvgDocument,
    width: u32,
    height: u32,
    elapsed_secs: f64,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("Failed to create Pixmap");

    // viewBox → 出力サイズへのスケーリング (アスペクト比を維持して中央配置)
    //
    //  出力 Pixmap (width x height)
    //  ┌──────────────────────────────┐
    //  │    ┌────────────────────┐    │
    //  │    │  viewBox 領域       │    │
    //  │    │  (scale 適用後)     │    │
    //  │    └────────────────────┘    │
    //  └──────────────────────────────┘
    let sx = width as f32 / doc.view_box.width;
    let sy = height as f32 / doc.view_box.height;
    let scale = sx.min(sy);
    let dx = (width as f32 - doc.view_box.width * scale) / 2.0 - doc.view_box.x * scale;
    let dy = (height as f32 - doc.view_box.height * scale) / 2.0 - doc.view_box.y * scale;
    let root_transform = Transform::from_row(scale, 0.0, 0.0, scale, dx, dy);

    let anim_state = AnimationState::new(&doc.animations, elapsed_secs);

    render_group(&mut pixmap, &doc.root, root_transform, 1.0, &anim_state);

    pixmap
}

/// SvgGroup を再帰的にレンダリングする。
fn render_group(
    pixmap: &mut Pixmap,
    group: &SvgGroup,
    parent_transform: Transform,
    parent_opacity: f32,
    anim_state: &AnimationState,
) {
    let group_transform = anim_state.resolve_transform(group.id.as_deref(), &group.transform);
    let group_opacity = anim_state.resolve_opacity(group.id.as_deref(), group.opacity);

    let transform = parent_transform.pre_concat(affine_to_transform(&group_transform));
    let opacity = parent_opacity * group_opacity;

    for child in &group.children {
        match child {
            SvgNode::Group(g) => render_group(pixmap, g, transform, opacity, anim_state),
            SvgNode::Path(p) => render_path(pixmap, p, transform, opacity, anim_state),
        }
    }
}

/// SvgPath を fill + stroke でレンダリングする。
fn render_path(
    pixmap: &mut Pixmap,
    path: &SvgPath,
    parent_transform: Transform,
    parent_opacity: f32,
    anim_state: &AnimationState,
) {
    let opacity = parent_opacity * anim_state.resolve_opacity(path.id.as_deref(), path.opacity);

    let ts_path = match build_tiny_path(&path.segments) {
        Some(p) => p,
        None => return,
    };

    // Fill
    if let Some(fill) = &path.fill {
        let resolved = anim_state.resolve_fill(path.id.as_deref(), fill);
        let alpha = (resolved.color.a as f32 * resolved.opacity * opacity).round() as u8;

        let mut paint = Paint::default();
        paint.set_color_rgba8(resolved.color.r, resolved.color.g, resolved.color.b, alpha);
        paint.anti_alias = true;

        let rule = match resolved.rule {
            FillRule::NonZero => tiny_skia::FillRule::Winding,
            FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
        };

        pixmap.fill_path(&ts_path, &paint, rule, parent_transform, None);
    }

    // Stroke
    if let Some(stroke) = &path.stroke {
        let resolved = anim_state.resolve_stroke(path.id.as_deref(), stroke);
        let alpha = (resolved.color.a as f32 * resolved.opacity * opacity).round() as u8;

        let mut paint = Paint::default();
        paint.set_color_rgba8(resolved.color.r, resolved.color.g, resolved.color.b, alpha);
        paint.anti_alias = true;

        let mut ts_stroke = Stroke::default();
        ts_stroke.width = resolved.width;
        ts_stroke.line_cap = match resolved.cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        };
        ts_stroke.line_join = match resolved.join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        };

        if let Some(dash) = &resolved.dash {
            let offset = anim_state.resolve_dash_offset(path.id.as_deref(), dash.offset);
            ts_stroke.dash = StrokeDash::new(dash.array.clone(), offset);
        }

        pixmap.stroke_path(&ts_path, &paint, &ts_stroke, parent_transform, None);
    }
}

/// svg::PathSegment 列を tiny_skia::Path に変換する。
fn build_tiny_path(segments: &[PathSegment]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for seg in segments {
        match *seg {
            PathSegment::MoveTo(x, y) => pb.move_to(x, y),
            PathSegment::LineTo(x, y) => pb.line_to(x, y),
            PathSegment::CubicTo {
                ctrl1_x,
                ctrl1_y,
                ctrl2_x,
                ctrl2_y,
                end_x,
                end_y,
            } => {
                pb.cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, end_x, end_y);
            }
            PathSegment::Close => pb.close(),
        }
    }
    pb.finish()
}

/// svg::Affine2D を tiny_skia::Transform に変換する。
fn affine_to_transform(affine: &Affine2D) -> Transform {
    Transform::from_row(affine.a, affine.b, affine.c, affine.d, affine.e, affine.f)
}

//! レンダリングエンジン。
//!
//! svg クレートのシーンツリーを tiny-skia で描画する。
//! パイプライン:
//!   1. viewBox → 出力サイズへのスケーリング変換を計算
//!   2. シーンツリーを再帰的に走査
//!   3. SvgGroup: transform を合成して子要素を描画
//!   4. SvgPath: PathSegment → tiny_skia::PathBuilder に変換し fill/stroke を描画

use svg::{
    Affine2D, FillRule, GradientUnits, LineCap, LineJoin, PathSegment, SpreadMethod, SvgDocument,
    SvgGroup, SvgNode, SvgPath,
};
use tiny_skia::{PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

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

    if let Some(clip_segments) = &group.clip_path {
        // グループにクリップパスがある場合:
        // 1. 一時 Pixmap に子要素を描画
        // 2. クリップマスクを適用して元の Pixmap に合成
        if let Some(mut temp) = Pixmap::new(pixmap.width(), pixmap.height()) {
            for child in &group.children {
                match child {
                    SvgNode::Group(g) => render_group(&mut temp, g, transform, opacity, anim_state),
                    SvgNode::Path(p) => render_path(&mut temp, p, transform, opacity, anim_state),
                }
            }
            if let Some(mask) = build_clip_mask(clip_segments, pixmap.width(), pixmap.height(), transform) {
                pixmap.draw_pixmap(
                    0,
                    0,
                    temp.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::identity(),
                    Some(&mask),
                );
            }
        }
    } else {
        for child in &group.children {
            match child {
                SvgNode::Group(g) => render_group(pixmap, g, transform, opacity, anim_state),
                SvgNode::Path(p) => render_path(pixmap, p, transform, opacity, anim_state),
            }
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
    // visibility="hidden" のパスは描画しない
    if !path.visibility {
        return;
    }

    let opacity = parent_opacity * anim_state.resolve_opacity(path.id.as_deref(), path.opacity);

    let ts_path = match build_tiny_path(&path.segments) {
        Some(p) => p,
        None => return,
    };

    // クリップマスクの構築
    let clip_mask = path.clip_path.as_ref().and_then(|clip_segments| {
        build_clip_mask(clip_segments, pixmap.width(), pixmap.height(), parent_transform)
    });
    let clip_ref = clip_mask.as_ref();

    // Fill
    if let Some(fill) = &path.fill {
        let resolved = anim_state.resolve_fill(path.id.as_deref(), fill);
        let rule = match resolved.rule {
            FillRule::NonZero => tiny_skia::FillRule::Winding,
            FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
        };
        match &resolved.paint {
            svg::Paint::Color(color) => {
                let alpha = (color.a as f32 * resolved.opacity * opacity).round() as u8;

                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(color.r, color.g, color.b, alpha);
                paint.anti_alias = true;

                pixmap.fill_path(&ts_path, &paint, rule, parent_transform, clip_ref);
            }
            svg::Paint::LinearGradient(grad) => {
                let bbox = compute_bbox(&path.segments);
                if let Some(shader) =
                    build_linear_gradient_shader(grad, resolved.opacity, opacity, bbox)
                {
                    let paint = tiny_skia::Paint {
                        shader,
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    };
                    pixmap.fill_path(&ts_path, &paint, rule, parent_transform, clip_ref);
                }
            }
            svg::Paint::RadialGradient(grad) => {
                let bbox = compute_bbox(&path.segments);
                if let Some(shader) =
                    build_radial_gradient_shader(grad, resolved.opacity, opacity, bbox)
                {
                    let paint = tiny_skia::Paint {
                        shader,
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    };
                    pixmap.fill_path(&ts_path, &paint, rule, parent_transform, clip_ref);
                }
            }
            svg::Paint::None => {}
        }
    }

    // Stroke
    if let Some(stroke) = &path.stroke {
        let resolved = anim_state.resolve_stroke(path.id.as_deref(), stroke);

        let mut ts_stroke = Stroke {
            width: resolved.width,
            line_cap: match resolved.cap {
                LineCap::Butt => tiny_skia::LineCap::Butt,
                LineCap::Round => tiny_skia::LineCap::Round,
                LineCap::Square => tiny_skia::LineCap::Square,
            },
            line_join: match resolved.join {
                LineJoin::Miter => tiny_skia::LineJoin::Miter,
                LineJoin::Round => tiny_skia::LineJoin::Round,
                LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
            },
            ..Stroke::default()
        };
        if let Some(dash) = &resolved.dash {
            let offset = anim_state.resolve_dash_offset(path.id.as_deref(), dash.offset);
            ts_stroke.dash = StrokeDash::new(dash.array.clone(), offset);
        }

        match &resolved.paint {
            svg::Paint::Color(color) => {
                let alpha = (color.a as f32 * resolved.opacity * opacity).round() as u8;

                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(color.r, color.g, color.b, alpha);
                paint.anti_alias = true;

                pixmap.stroke_path(&ts_path, &paint, &ts_stroke, parent_transform, clip_ref);
            }
            svg::Paint::LinearGradient(grad) => {
                let bbox = compute_bbox(&path.segments);
                if let Some(shader) =
                    build_linear_gradient_shader(grad, resolved.opacity, opacity, bbox)
                {
                    let paint = tiny_skia::Paint {
                        shader,
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    };
                    pixmap.stroke_path(&ts_path, &paint, &ts_stroke, parent_transform, clip_ref);
                }
            }
            svg::Paint::RadialGradient(grad) => {
                let bbox = compute_bbox(&path.segments);
                if let Some(shader) =
                    build_radial_gradient_shader(grad, resolved.opacity, opacity, bbox)
                {
                    let paint = tiny_skia::Paint {
                        shader,
                        anti_alias: true,
                        ..tiny_skia::Paint::default()
                    };
                    pixmap.stroke_path(&ts_path, &paint, &ts_stroke, parent_transform, clip_ref);
                }
            }
            svg::Paint::None => {}
        }
    }
}

/// クリップパスセグメントから tiny-skia Mask を構築する。
/// クリップ対象の描画領域サイズと変換行列を受け取り、マスクを返す。
fn build_clip_mask(
    clip_segments: &[PathSegment],
    width: u32,
    height: u32,
    transform: Transform,
) -> Option<tiny_skia::Mask> {
    let clip_path = build_tiny_path(clip_segments)?;
    let mut mask = tiny_skia::Mask::new(width, height)?;
    mask.fill_path(&clip_path, tiny_skia::FillRule::Winding, true, transform);
    Some(mask)
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

/// パスセグメントのバウンディングボックスを計算する。
/// 戻り値は (x, y, width, height)。
fn compute_bbox(segments: &[PathSegment]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for seg in segments {
        let (x, y) = match *seg {
            PathSegment::MoveTo(x, y) | PathSegment::LineTo(x, y) => (x, y),
            PathSegment::CubicTo {
                end_x, end_y, ..
            } => (end_x, end_y),
            PathSegment::Close => continue,
        };
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    if min_x > max_x {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// svg::LinearGradient から tiny-skia の Shader を構築する。
fn build_linear_gradient_shader(
    grad: &svg::LinearGradient,
    fill_opacity: f32,
    parent_opacity: f32,
    bbox: (f32, f32, f32, f32),
) -> Option<tiny_skia::Shader<'static>> {
    let (bx, by, bw, bh) = bbox;

    let (x1, y1, x2, y2) = match grad.units {
        GradientUnits::ObjectBoundingBox => {
            // objectBoundingBox: 0-1 の値をバウンディングボックスにマッピング
            (
                bx + grad.x1 * bw,
                by + grad.y1 * bh,
                bx + grad.x2 * bw,
                by + grad.y2 * bh,
            )
        }
        GradientUnits::UserSpaceOnUse => (grad.x1, grad.y1, grad.x2, grad.y2),
    };

    let stops: Vec<tiny_skia::GradientStop> = grad
        .stops
        .iter()
        .map(|s| {
            let a = (s.color.a as f32 * s.opacity * fill_opacity * parent_opacity).round() as u8;
            tiny_skia::GradientStop::new(
                s.offset,
                tiny_skia::Color::from_rgba8(s.color.r, s.color.g, s.color.b, a),
            )
        })
        .collect();

    if stops.len() < 2 {
        return None;
    }

    let spread = match grad.spread {
        SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    };

    let transform = affine_to_transform(&grad.transform);

    tiny_skia::LinearGradient::new(
        tiny_skia::Point::from_xy(x1, y1),
        tiny_skia::Point::from_xy(x2, y2),
        stops,
        spread,
        transform,
    )
}

/// svg::RadialGradient から tiny-skia の Shader を構築する。
fn build_radial_gradient_shader(
    grad: &svg::RadialGradient,
    fill_opacity: f32,
    parent_opacity: f32,
    bbox: (f32, f32, f32, f32),
) -> Option<tiny_skia::Shader<'static>> {
    let (bx, by, bw, bh) = bbox;

    let (cx, cy, r, fx, fy) = match grad.units {
        GradientUnits::ObjectBoundingBox => {
            // objectBoundingBox: 0-1 の値をバウンディングボックスにマッピング
            // 半径は幅と高さの大きい方を基準にする
            (
                bx + grad.cx * bw,
                by + grad.cy * bh,
                grad.r * bw.max(bh),
                bx + grad.fx * bw,
                by + grad.fy * bh,
            )
        }
        GradientUnits::UserSpaceOnUse => (grad.cx, grad.cy, grad.r, grad.fx, grad.fy),
    };

    let stops: Vec<tiny_skia::GradientStop> = grad
        .stops
        .iter()
        .map(|s| {
            let a = (s.color.a as f32 * s.opacity * fill_opacity * parent_opacity).round() as u8;
            tiny_skia::GradientStop::new(
                s.offset,
                tiny_skia::Color::from_rgba8(s.color.r, s.color.g, s.color.b, a),
            )
        })
        .collect();

    if stops.len() < 2 {
        return None;
    }

    let spread = match grad.spread {
        SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    };

    let transform = affine_to_transform(&grad.transform);

    // tiny-skia の RadialGradient::new:
    //   start (focal point), end (center), radius, stops, mode, transform
    tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(fx, fy),
        tiny_skia::Point::from_xy(cx, cy),
        r,
        stops,
        spread,
        transform,
    )
}

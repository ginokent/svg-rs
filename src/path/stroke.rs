use crate::types::*;

/// ストロークパスをフィルパス (アウトライン) に変換する。
///
/// 処理フロー:
/// 1. DashPattern が指定されている場合はパスをダッシュ区間に分割
/// 2. パスを折れ線にフラッテニング
/// 3. 各折れ線セグメントに対して法線方向にオフセットした外形/内形パスを生成
/// 4. LineCap/LineJoin に応じた端点/屈折点の処理
pub fn stroke_to_fill(segments: &[PathSegment], style: &StrokeStyle) -> Vec<PathSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let half_width = style.width / 2.0;
    if half_width <= 0.0 {
        return Vec::new();
    }

    // ダッシュパターンの適用
    let segments = if let Some(ref dash) = style.dash {
        apply_dash(segments, dash)
    } else {
        segments.to_vec()
    };

    // フラッテニングして折れ線に変換
    let subpaths = super::flatten::flatten(&segments, 0.25);

    let mut result = Vec::new();

    for subpath in &subpaths {
        if subpath.len() < 2 {
            continue;
        }

        let is_closed = subpath.first() == subpath.last() && subpath.len() > 2;
        let outline = build_stroke_outline(subpath, half_width, is_closed, style.cap, style.join);
        result.extend(outline);
    }

    result
}

/// 折れ線パスのストロークアウトラインを構築する。
///
/// ```text
///     外形パス (右側)
///   ─────────────────────→
///   ← ← ← ← ← ← ← ← ← ←
///     内形パス (左側, 逆順)
/// ```
///
/// 外形パスを正方向、内形パスを逆方向に結合して閉じたアウトラインを生成する。
fn build_stroke_outline(
    points: &[(f32, f32)],
    half_width: f32,
    is_closed: bool,
    cap: LineCap,
    join: LineJoin,
) -> Vec<PathSegment> {
    let n = if is_closed {
        points.len() - 1 // 閉じたパスでは最後の重複点を除く
    } else {
        points.len()
    };

    if n < 2 {
        return Vec::new();
    }

    // 各セグメントの法線ベクトルを計算
    //
    // 開いたパス: n-1 本のセグメント → n-1 個の法線
    // 閉じたパス: n 本のセグメント (閉じるセグメント含む) → n 個の法線
    let normal_count = if is_closed { n } else { n - 1 };
    let mut normals: Vec<(f32, f32)> = Vec::with_capacity(normal_count);
    for i in 0..n - 1 {
        let dx = points[i + 1].0 - points[i].0;
        let dy = points[i + 1].1 - points[i].1;
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            normals.push((-dy / len, dx / len));
        } else {
            // 同一点のセグメントには前の法線を使う
            normals.push(if let Some(&prev) = normals.last() { prev } else { (0.0, 1.0) });
        }
    }
    // 閉じたパスの場合、閉じるセグメント (最後の頂点→最初の頂点) の法線も計算
    if is_closed {
        let dx = points[0].0 - points[n - 1].0;
        let dy = points[0].1 - points[n - 1].1;
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            normals.push((-dy / len, dx / len));
        } else {
            normals.push(*normals.last().unwrap_or(&(0.0, 1.0)));
        }
    }

    if normals.is_empty() {
        return Vec::new();
    }

    // 外形 (右側) の点列
    let mut outer: Vec<(f32, f32)> = Vec::with_capacity(n * 2);
    // 内形 (左側) の点列
    let mut inner: Vec<(f32, f32)> = Vec::with_capacity(n * 2);

    if is_closed {
        // 閉じたパス: 最初の頂点も join 処理で追加する (単純オフセットではなく)
        // v0 の join: 閉じるセグメント (v_{n-1}→v0) と最初のセグメント (v0→v1) の接合
        add_join_points(points[0], normals[n - 1], normals[0], half_width, join, &mut outer, &mut inner);

        // 内部頂点 v1..v_{n-1} の join 処理 (v_{n-1} も含む — 閉じるセグメントとの接合)
        for i in 1..n {
            let n_prev = normals[i - 1];
            let n_next = normals[i];
            let p = points[i];
            add_join_points(p, n_prev, n_next, half_width, join, &mut outer, &mut inner);
        }
    } else {
        // 開いたパス: 最初のセグメントの始点
        let n0 = normals[0];
        outer.push((
            points[0].0 + n0.0 * half_width,
            points[0].1 + n0.1 * half_width,
        ));
        inner.push((
            points[0].0 - n0.0 * half_width,
            points[0].1 - n0.1 * half_width,
        ));

        // 各屈折点を処理
        for i in 1..n - 1 {
            let n_prev = normals[i - 1];
            let n_next = normals[i];
            let p = points[i];
            add_join_points(p, n_prev, n_next, half_width, join, &mut outer, &mut inner);
        }

        // 開いたパスの終点
        let n_last = *normals.last().unwrap();
        let last_pt = points[n - 1];
        outer.push((
            last_pt.0 + n_last.0 * half_width,
            last_pt.1 + n_last.1 * half_width,
        ));
        inner.push((
            last_pt.0 - n_last.0 * half_width,
            last_pt.1 - n_last.1 * half_width,
        ));
    }

    // PathSegment 列を構築
    let mut segs = Vec::new();

    if is_closed {
        // 閉じたパスの場合は外形と内形を別々の閉じたパスとして出力
        // 外形パス (時計回り)
        if let Some(&first) = outer.first() {
            segs.push(PathSegment::MoveTo(first.0, first.1));
            for &pt in &outer[1..] {
                segs.push(PathSegment::LineTo(pt.0, pt.1));
            }
            segs.push(PathSegment::Close);
        }

        // 内形パス (反時計回り = 穴)
        if let Some(&first) = inner.last() {
            segs.push(PathSegment::MoveTo(first.0, first.1));
            for &pt in inner.iter().rev().skip(1) {
                segs.push(PathSegment::LineTo(pt.0, pt.1));
            }
            segs.push(PathSegment::Close);
        }
    } else {
        // 開いたパスの場合: 外形 → 終点キャップ → 内形 (逆順) → 始点キャップ → Close
        if let Some(&first) = outer.first() {
            segs.push(PathSegment::MoveTo(first.0, first.1));
            for &pt in &outer[1..] {
                segs.push(PathSegment::LineTo(pt.0, pt.1));
            }
        }

        // 終点キャップ
        let end_pt = points[n - 1];
        let n_last = *normals.last().unwrap();
        add_cap(&mut segs, end_pt, n_last, half_width, cap, false);

        // 内形パスを逆順に追加
        for &pt in inner.iter().rev() {
            segs.push(PathSegment::LineTo(pt.0, pt.1));
        }

        // 始点キャップ
        let start_pt = points[0];
        let n_first = normals[0];
        add_cap(&mut segs, start_pt, n_first, half_width, cap, true);

        segs.push(PathSegment::Close);
    }

    segs
}

/// 屈折点における外形と内形の点を追加する。
///
/// Bevel/Round join では凸側と凹側で処理を分ける:
/// - **凸側** (開く側): 指定された join スタイルを適用 (2 点)
/// - **凹側** (閉じる側): 常に miter 交点 (1 点) を使用し自己交差を防ぐ
///
/// ```text
///   outer(+normal 方向)
///     ●───────●  ← 凸側: Bevel/Round (2 点で面取り)
///    ╱         ╲
///   P ─────────→ 次セグメント
///    ╲         ╱
///     ●           ← 凹側: Miter (1 点で交差回避)
///   inner(-normal 方向)
/// ```
fn add_join_points(
    p: (f32, f32),
    n_prev: (f32, f32),
    n_next: (f32, f32),
    half_width: f32,
    join: LineJoin,
    outer: &mut Vec<(f32, f32)>,
    inner: &mut Vec<(f32, f32)>,
) {
    match join {
        LineJoin::Bevel | LineJoin::Round => {
            // 凸側/凹側の判定: 法線ベクトルの外積
            //   cross > 0 → outer 側が凸、inner 側が凹
            //   cross < 0 → outer 側が凹、inner 側が凸
            //   cross ≈ 0 → ほぼ直線
            let cross = n_prev.0 * n_next.1 - n_prev.1 * n_next.0;

            if cross > 1e-6 {
                // outer が凸 → join スタイル適用 (2 点), inner が凹 → miter (1 点)
                outer.push((p.0 + n_prev.0 * half_width, p.1 + n_prev.1 * half_width));
                outer.push((p.0 + n_next.0 * half_width, p.1 + n_next.1 * half_width));
                // 凹側: miter 交点を求める。miter_limit 超過時は屈折点 p 自体をフォールバック
                let inner_pt = miter_point(
                    p,
                    (-n_prev.0, -n_prev.1),
                    (-n_next.0, -n_next.1),
                    half_width,
                    f32::MAX, // 凹側は miter_limit を無制限にする (交点は元の点に近い)
                ).unwrap_or(p);
                inner.push(inner_pt);
            } else if cross < -1e-6 {
                // outer が凹 → miter (1 点), inner が凸 → join スタイル適用 (2 点)
                let outer_pt = miter_point(
                    p,
                    n_prev,
                    n_next,
                    half_width,
                    f32::MAX,
                ).unwrap_or(p);
                outer.push(outer_pt);
                inner.push((p.0 - n_prev.0 * half_width, p.1 - n_prev.1 * half_width));
                inner.push((p.0 - n_next.0 * half_width, p.1 - n_next.1 * half_width));
            } else {
                // ほぼ直線 → 各側 1 点ずつ (n_next のオフセット)
                outer.push((p.0 + n_next.0 * half_width, p.1 + n_next.1 * half_width));
                inner.push((p.0 - n_next.0 * half_width, p.1 - n_next.1 * half_width));
            }
        }
        LineJoin::Miter => {
            // Miter: 法線の延長線の交点を求める
            // 交点が見つからない場合 (平行) は Bevel にフォールバック
            let miter_limit = 4.0; // SVG デフォルト
            if let Some(op) = miter_point(p, n_prev, n_next, half_width, miter_limit) {
                outer.push(op);
            } else {
                outer.push((p.0 + n_prev.0 * half_width, p.1 + n_prev.1 * half_width));
                outer.push((p.0 + n_next.0 * half_width, p.1 + n_next.1 * half_width));
            }
            if let Some(ip) = miter_point(p, (-n_prev.0, -n_prev.1), (-n_next.0, -n_next.1), half_width, miter_limit) {
                inner.push(ip);
            } else {
                inner.push((p.0 - n_prev.0 * half_width, p.1 - n_prev.1 * half_width));
                inner.push((p.0 - n_next.0 * half_width, p.1 - n_next.1 * half_width));
            }
        }
    }
}

/// Miter join の交点を計算する。
/// miter_limit を超える場合は None を返す (Bevel にフォールバック)。
fn miter_point(
    p: (f32, f32),
    n_prev: (f32, f32),
    n_next: (f32, f32),
    half_width: f32,
    miter_limit: f32,
) -> Option<(f32, f32)> {
    let p1 = (p.0 + n_prev.0 * half_width, p.1 + n_prev.1 * half_width);
    let p2 = (p.0 + n_next.0 * half_width, p.1 + n_next.1 * half_width);

    // 法線に直交する方向 (パス進行方向)
    let d1 = (-n_prev.1, n_prev.0);
    let d2 = (-n_next.1, n_next.0);

    // 2 直線の交点: p1 + t*d1 = p2 + s*d2
    let cross = d1.0 * d2.1 - d1.1 * d2.0;
    if cross.abs() < 1e-10 {
        return None; // 平行
    }

    let t = ((p2.0 - p1.0) * d2.1 - (p2.1 - p1.1) * d2.0) / cross;
    let intersection = (p1.0 + t * d1.0, p1.1 + t * d1.1);

    // miter length チェック
    let dx = intersection.0 - p.0;
    let dy = intersection.1 - p.1;
    let miter_length = (dx * dx + dy * dy).sqrt();
    if miter_length > half_width * miter_limit {
        return None;
    }

    Some(intersection)
}

/// パス端点のキャップを追加する。
/// `is_start` が true なら始点側、false なら終点側。
fn add_cap(
    segs: &mut Vec<PathSegment>,
    point: (f32, f32),
    normal: (f32, f32),
    half_width: f32,
    cap: LineCap,
    is_start: bool,
) {
    match cap {
        LineCap::Butt => {
            // 何もしない (直角カット)
        }
        LineCap::Square => {
            // stroke-width/2 だけ進行方向に延長
            let dir = if is_start {
                // 始点: 進行方向の反対
                (normal.1, -normal.0)
            } else {
                // 終点: 進行方向
                (-normal.1, normal.0)
            };
            let ext_outer = (
                point.0 + normal.0 * half_width + dir.0 * half_width,
                point.1 + normal.1 * half_width + dir.1 * half_width,
            );
            let ext_inner = (
                point.0 - normal.0 * half_width + dir.0 * half_width,
                point.1 - normal.1 * half_width + dir.1 * half_width,
            );
            segs.push(PathSegment::LineTo(ext_outer.0, ext_outer.1));
            segs.push(PathSegment::LineTo(ext_inner.0, ext_inner.1));
        }
        LineCap::Round => {
            // 半円を CubicTo で近似
            let k = 0.5522847498_f32;
            let dir = if is_start {
                (normal.1, -normal.0)
            } else {
                (-normal.1, normal.0)
            };

            let p_outer = (
                point.0 + normal.0 * half_width,
                point.1 + normal.1 * half_width,
            );
            let p_inner = (
                point.0 - normal.0 * half_width,
                point.1 - normal.1 * half_width,
            );
            let p_tip = (
                point.0 + dir.0 * half_width,
                point.1 + dir.1 * half_width,
            );

            // outer → tip
            segs.push(PathSegment::CubicTo {
                ctrl1_x: p_outer.0 + dir.0 * half_width * k,
                ctrl1_y: p_outer.1 + dir.1 * half_width * k,
                ctrl2_x: p_tip.0 + normal.0 * half_width * k,
                ctrl2_y: p_tip.1 + normal.1 * half_width * k,
                end_x: p_tip.0,
                end_y: p_tip.1,
            });
            // tip → inner
            segs.push(PathSegment::CubicTo {
                ctrl1_x: p_tip.0 - normal.0 * half_width * k,
                ctrl1_y: p_tip.1 - normal.1 * half_width * k,
                ctrl2_x: p_inner.0 + dir.0 * half_width * k,
                ctrl2_y: p_inner.1 + dir.1 * half_width * k,
                end_x: p_inner.0,
                end_y: p_inner.1,
            });
        }
    }
}

/// DashPattern を適用してパスをダッシュ区間に分割する。
fn apply_dash(segments: &[PathSegment], dash: &DashPattern) -> Vec<PathSegment> {
    if dash.array.is_empty() {
        return segments.to_vec();
    }

    // フラッテニングして折れ線に変換
    let subpaths = super::flatten::flatten(segments, 0.25);
    let mut result = Vec::new();

    for subpath in &subpaths {
        if subpath.len() < 2 {
            continue;
        }

        // パスの各セグメントの累積長を計算
        let mut _total_length = 0.0_f32;
        for i in 1..subpath.len() {
            let dx = subpath[i].0 - subpath[i - 1].0;
            let dy = subpath[i].1 - subpath[i - 1].1;
            _total_length += (dx * dx + dy * dy).sqrt();
        }

        // ダッシュパターンの 1 サイクル長
        let pattern_len: f32 = dash.array.iter().sum();
        if pattern_len <= 0.0 {
            // パターン長 0 の場合は元のパスをそのまま返す
            result.extend(subpath_to_segments(subpath));
            continue;
        }

        // ダッシュ/ギャップ区間を切り出す
        let offset = dash.offset % pattern_len;
        let mut dash_pos = -offset; // 現在のダッシュパターン内位置
        let mut dash_idx = 0; // ダッシュ配列のインデックス
        let mut is_drawing = true; // true=描画区間, false=非描画区間

        // offset 分を進める
        if offset > 0.0 {
            let mut remaining = offset;
            while remaining > 0.0 && !dash.array.is_empty() {
                let seg_len = dash.array[dash_idx % dash.array.len()];
                if remaining >= seg_len {
                    remaining -= seg_len;
                    is_drawing = !is_drawing;
                    dash_idx += 1;
                } else {
                    dash_pos = remaining - seg_len;
                    break;
                }
            }
        }

        // 折れ線上を歩いてダッシュ区間を切り出す
        let _current_seg_start = 0;
        let _seg_offset = 0.0_f32;
        let _accumulated = 0.0_f32;
        let mut current_dash_remaining =
            dash.array[dash_idx % dash.array.len()] + dash_pos;

        let mut dash_subpath: Vec<(f32, f32)> = Vec::new();

        for i in 1..subpath.len() {
            let dx = subpath[i].0 - subpath[i - 1].0;
            let dy = subpath[i].1 - subpath[i - 1].1;
            let seg_len = (dx * dx + dy * dy).sqrt();

            if seg_len == 0.0 {
                continue;
            }

            let mut pos_in_seg = 0.0_f32;

            while pos_in_seg < seg_len {
                let remaining_in_seg = seg_len - pos_in_seg;

                if current_dash_remaining <= remaining_in_seg {
                    // ダッシュ/ギャップの境界がこのセグメント内にある
                    let t = (pos_in_seg + current_dash_remaining) / seg_len;
                    let split_point = (
                        subpath[i - 1].0 + dx * t,
                        subpath[i - 1].1 + dy * t,
                    );

                    if is_drawing {
                        if dash_subpath.is_empty() {
                            dash_subpath.push(interpolate_point(subpath[i - 1], subpath[i], pos_in_seg / seg_len));
                        }
                        dash_subpath.push(split_point);
                        // ダッシュ区間が終了 → 出力
                        result.extend(subpath_to_segments(&dash_subpath));
                        dash_subpath.clear();
                    }

                    pos_in_seg += current_dash_remaining;
                    is_drawing = !is_drawing;
                    dash_idx += 1;
                    current_dash_remaining = dash.array[dash_idx % dash.array.len()];
                } else {
                    // このセグメントの終端まで到達
                    current_dash_remaining -= remaining_in_seg;

                    if is_drawing {
                        if dash_subpath.is_empty() {
                            dash_subpath.push(interpolate_point(subpath[i - 1], subpath[i], pos_in_seg / seg_len));
                        }
                        dash_subpath.push(subpath[i]);
                    }

                    pos_in_seg = seg_len;
                }
            }
        }

        // 最後のダッシュ区間が残っていれば出力
        if is_drawing && dash_subpath.len() >= 2 {
            result.extend(subpath_to_segments(&dash_subpath));
        }
    }

    result
}

fn interpolate_point(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn subpath_to_segments(points: &[(f32, f32)]) -> Vec<PathSegment> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut segs = Vec::with_capacity(points.len());
    segs.push(PathSegment::MoveTo(points[0].0, points[0].1));
    for &(x, y) in &points[1..] {
        segs.push(PathSegment::LineTo(x, y));
    }
    segs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stroke(width: f32) -> StrokeStyle {
        StrokeStyle {
            paint: Paint::Color(Color { r: 0, g: 0, b: 0, a: 255 }),
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: None,
            opacity: 1.0,
        }
    }

    #[test]
    fn test_empty_segments() {
        let result = stroke_to_fill(&[], &make_stroke(2.0));
        assert!(result.is_empty());
    }

    #[test]
    fn test_zero_width() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
        ];
        let result = stroke_to_fill(&segs, &make_stroke(0.0));
        assert!(result.is_empty());
    }

    #[test]
    fn test_simple_horizontal_line() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
        ];
        let result = stroke_to_fill(&segs, &make_stroke(2.0));
        // Should produce a rectangle-like outline
        assert!(!result.is_empty());
        // 最初は MoveTo
        assert!(matches!(result[0], PathSegment::MoveTo(_, _)));
        // 最後は Close
        assert!(matches!(result.last(), Some(PathSegment::Close)));
    }

    #[test]
    fn test_closed_path() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
            PathSegment::LineTo(10.0, 10.0),
            PathSegment::Close,
        ];
        let style = make_stroke(2.0);
        let result = stroke_to_fill(&segs, &style);
        assert!(!result.is_empty());
        // 閉じたパスは外形と内形の 2 つの閉じたパスを出力
        let close_count = result.iter().filter(|s| matches!(s, PathSegment::Close)).count();
        assert_eq!(close_count, 2);
    }

    #[test]
    fn test_dash_pattern() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(100.0, 0.0),
        ];
        let mut style = make_stroke(2.0);
        style.dash = Some(DashPattern {
            array: vec![10.0, 10.0],
            offset: 0.0,
        });
        let result = stroke_to_fill(&segs, &style);
        // ダッシュ区間が分割されるので複数の MoveTo/Close が存在する
        let move_count = result.iter().filter(|s| matches!(s, PathSegment::MoveTo(_, _))).count();
        assert!(move_count >= 2, "Dash pattern should produce multiple subpaths, got {}", move_count);
    }

    #[test]
    fn test_square_cap() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
        ];
        let mut style = make_stroke(2.0);
        style.cap = LineCap::Square;
        let result = stroke_to_fill(&segs, &style);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_round_cap() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
        ];
        let mut style = make_stroke(2.0);
        style.cap = LineCap::Round;
        let result = stroke_to_fill(&segs, &style);
        assert!(!result.is_empty());
        // Round cap は CubicTo を使う
        let cubic_count = result.iter().filter(|s| matches!(s, PathSegment::CubicTo { .. })).count();
        assert!(cubic_count > 0, "Round cap should produce CubicTo segments");
    }

    #[test]
    fn test_bevel_join() {
        let segs = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
            PathSegment::LineTo(10.0, 10.0),
        ];
        let mut style = make_stroke(2.0);
        style.join = LineJoin::Bevel;
        let result = stroke_to_fill(&segs, &style);
        assert!(!result.is_empty());
    }
}

use crate::types::PathSegment;

/// 再帰の最大深度。無限再帰を防止するためのガード。
const MAX_RECURSION_DEPTH: u32 = 16;

/// サブパスごとにフラッテニングした結果を返す。
///
/// 3 次ベジェ曲線を adaptive de Casteljau subdivision で折れ線に近似し、
/// 各サブパスを `Vec<(f32, f32)>` として返す。
pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<(f32, f32)>> {
    let mut result: Vec<Vec<(f32, f32)>> = Vec::new();
    // 現在構築中のサブパス
    let mut current_subpath: Vec<(f32, f32)> = Vec::new();
    // 現在のサブパスの開始点 (Close で戻る先)
    let mut subpath_start: (f32, f32) = (0.0, 0.0);
    // 現在のカーソル位置
    let mut cursor: (f32, f32) = (0.0, 0.0);

    for seg in segments {
        match *seg {
            PathSegment::MoveTo(x, y) => {
                // 既存のサブパスがあればファイナライズ
                if !current_subpath.is_empty() {
                    result.push(current_subpath);
                    current_subpath = Vec::new();
                }
                // 新しいサブパスを開始
                subpath_start = (x, y);
                cursor = (x, y);
                current_subpath.push((x, y));
            }
            PathSegment::LineTo(x, y) => {
                current_subpath.push((x, y));
                cursor = (x, y);
            }
            PathSegment::CubicTo {
                ctrl1_x,
                ctrl1_y,
                ctrl2_x,
                ctrl2_y,
                end_x,
                end_y,
            } => {
                // adaptive de Casteljau subdivision で 3 次ベジェ曲線をフラッテニング
                flatten_cubic(
                    cursor,
                    (ctrl1_x, ctrl1_y),
                    (ctrl2_x, ctrl2_y),
                    (end_x, end_y),
                    tolerance,
                    &mut current_subpath,
                    0,
                );
                cursor = (end_x, end_y);
            }
            PathSegment::Close => {
                // サブパスの始点を追加して閉じる
                current_subpath.push(subpath_start);
                result.push(current_subpath);
                current_subpath = Vec::new();
                cursor = subpath_start;
            }
        }
    }

    // 最後のサブパスが残っていればファイナライズ
    if !current_subpath.is_empty() {
        result.push(current_subpath);
    }

    result
}

/// 3 次ベジェ曲線 (p0, p1, p2, p3) を adaptive de Casteljau subdivision でフラッテニング。
///
/// ```text
/// p1 ●-------● p2
///   /           \
///  /             \
/// ● p0           ● p3
/// ```
///
/// 制御点 p1, p2 が直線 p0-p3 から tolerance 以下しか離れていなければ
/// 十分に平坦とみなし、p3 のみを出力する。
/// そうでなければ t=0.5 で分割して再帰的に処理する。
fn flatten_cubic(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    tolerance: f32,
    output: &mut Vec<(f32, f32)>,
    depth: u32,
) {
    // 再帰深度の上限に達した場合は強制的に直線近似
    if depth >= MAX_RECURSION_DEPTH {
        output.push(p3);
        return;
    }

    // 平坦性テスト: 制御点 p1, p2 が直線 p0-p3 からどれだけ離れているか
    let d1 = point_to_line_distance(p1, p0, p3);
    let d2 = point_to_line_distance(p2, p0, p3);

    if d1.max(d2) <= tolerance {
        // 十分に平坦 → 直線で近似
        output.push(p3);
        return;
    }

    // de Casteljau 分割 (t=0.5)
    //
    //   p0 ---- p01 ---- p012 ---- p0123 (分割点)
    //   p1 ---- p12 ---- p123
    //   p2 ---- p23
    //   p3
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123); // 分割点

    // 左半分: (p0, p01, p012, p0123)
    flatten_cubic(p0, p01, p012, p0123, tolerance, output, depth + 1);
    // 右半分: (p0123, p123, p23, p3)
    flatten_cubic(p0123, p123, p23, p3, tolerance, output, depth + 1);
}

/// 2 点の中点を返す。
#[inline]
fn midpoint(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

/// 点 p から直線 (line_start → line_end) への距離を返す。
///
/// 直線が退化している場合 (line_start == line_end) は点間距離を返す。
fn point_to_line_distance(p: (f32, f32), line_start: (f32, f32), line_end: (f32, f32)) -> f32 {
    let dx = line_end.0 - line_start.0;
    let dy = line_end.1 - line_start.1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        // 直線が退化 → 点間距離
        let ex = p.0 - line_start.0;
        let ey = p.1 - line_start.1;
        return (ex * ex + ey * ey).sqrt();
    }

    // |dy * px - dx * py + x2 * y1 - y2 * x1| / sqrt(dx^2 + dy^2)
    let numerator =
        (dy * p.0 - dx * p.1 + line_end.0 * line_start.1 - line_end.1 * line_start.0).abs();
    numerator / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PathSegment;

    /// 空入力 → 空の結果
    #[test]
    fn test_empty_input() {
        let result = flatten(&[], 1.0);
        assert!(result.is_empty());
    }

    /// LineTo のみ → 分割なしでそのまま出力
    #[test]
    fn test_line_segments_no_subdivision() {
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
            PathSegment::LineTo(10.0, 10.0),
        ];
        let result = flatten(&segments, 1.0);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]
        );
    }

    /// 単一の CubicTo → 複数の点が生成される
    #[test]
    fn test_single_cubic_produces_multiple_points() {
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::CubicTo {
                ctrl1_x: 0.0,
                ctrl1_y: 100.0,
                ctrl2_x: 100.0,
                ctrl2_y: 100.0,
                end_x: 100.0,
                end_y: 0.0,
            },
        ];
        let result = flatten(&segments, 0.5);
        assert_eq!(result.len(), 1);
        // MoveTo の 1 点 + CubicTo のフラッテニング結果 (複数点)
        assert!(
            result[0].len() > 2,
            "CubicTo should produce more than 2 points, got {}",
            result[0].len()
        );
        // 始点と終点を検証
        assert_eq!(result[0].first(), Some(&(0.0, 0.0)));
        assert_eq!(result[0].last(), Some(&(100.0, 0.0)));
    }

    /// tolerance が小さいほど出力点数が多くなる
    #[test]
    fn test_tolerance_affects_point_count() {
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::CubicTo {
                ctrl1_x: 0.0,
                ctrl1_y: 100.0,
                ctrl2_x: 100.0,
                ctrl2_y: 100.0,
                end_x: 100.0,
                end_y: 0.0,
            },
        ];

        let coarse = flatten(&segments, 10.0);
        let fine = flatten(&segments, 0.1);

        assert!(
            fine[0].len() > coarse[0].len(),
            "Smaller tolerance should produce more points: fine={} coarse={}",
            fine[0].len(),
            coarse[0].len()
        );
    }

    /// MoveTo が新しいサブパスを開始する
    #[test]
    fn test_moveto_starts_new_subpath() {
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 10.0),
            PathSegment::MoveTo(20.0, 20.0),
            PathSegment::LineTo(30.0, 30.0),
        ];
        let result = flatten(&segments, 1.0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![(0.0, 0.0), (10.0, 10.0)]);
        assert_eq!(result[1], vec![(20.0, 20.0), (30.0, 30.0)]);
    }

    /// Close がサブパスを閉じる (始点が末尾に追加される)
    #[test]
    fn test_close_closes_subpath() {
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
            PathSegment::LineTo(10.0, 10.0),
            PathSegment::Close,
        ];
        let result = flatten(&segments, 1.0);
        assert_eq!(result.len(), 1);
        // Close により始点 (0,0) が末尾に追加される
        assert_eq!(
            result[0],
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)]
        );
    }

    /// 複数のサブパス (Close で分割)
    #[test]
    fn test_multiple_subpaths() {
        let segments = vec![
            // サブパス 1: 三角形
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::LineTo(10.0, 0.0),
            PathSegment::LineTo(5.0, 10.0),
            PathSegment::Close,
            // サブパス 2: 別の三角形
            PathSegment::MoveTo(20.0, 20.0),
            PathSegment::LineTo(30.0, 20.0),
            PathSegment::LineTo(25.0, 30.0),
            PathSegment::Close,
        ];
        let result = flatten(&segments, 1.0);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0), (0.0, 0.0)]
        );
        assert_eq!(
            result[1],
            vec![(20.0, 20.0), (30.0, 20.0), (25.0, 30.0), (20.0, 20.0)]
        );
    }

    /// point_to_line_distance の基本テスト
    #[test]
    fn test_point_to_line_distance() {
        // 水平線 (0,0)→(10,0) から点 (5,3) への距離 = 3.0
        let d = point_to_line_distance((5.0, 3.0), (0.0, 0.0), (10.0, 0.0));
        assert!((d - 3.0).abs() < 1e-5);

        // 退化した直線 (同一点) → 点間距離
        let d = point_to_line_distance((3.0, 4.0), (0.0, 0.0), (0.0, 0.0));
        assert!((d - 5.0).abs() < 1e-5);
    }

    /// 直線的な CubicTo (制御点が直線上) → 分割なしで 1 点のみ追加
    #[test]
    fn test_linear_cubic_no_subdivision() {
        // 制御点が p0-p3 の直線上にある場合、分割は不要
        let segments = vec![
            PathSegment::MoveTo(0.0, 0.0),
            PathSegment::CubicTo {
                ctrl1_x: 10.0,
                ctrl1_y: 0.0,
                ctrl2_x: 20.0,
                ctrl2_y: 0.0,
                end_x: 30.0,
                end_y: 0.0,
            },
        ];
        let result = flatten(&segments, 1.0);
        assert_eq!(result.len(), 1);
        // MoveTo + 終点のみ (制御点が直線上なので分割不要)
        assert_eq!(result[0], vec![(0.0, 0.0), (30.0, 0.0)]);
    }
}

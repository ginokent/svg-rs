use crate::types::PathSegment;
use crate::xml::XmlElement;

/// XmlElement の数値属性を f32 として取得する。属性が存在しない / パース失敗時は default を返す。
fn attr_f32(el: &XmlElement, name: &str, default: f32) -> f32 {
    el.attribute(name)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default)
}

/// SVG の points 属性 ("x1,y1 x2,y2 ..." or "x1 y1 x2 y2 ...") を座標ペア列にパースする。
///
/// 区切り文字としてカンマ・スペース・タブ・改行を許容する。
/// 座標は連続する 2 つの数値を 1 ペアとして扱い、奇数個の場合は末尾の余りを無視する。
fn parse_points(s: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = s
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();

    nums.chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

/// 円弧の四分円近似に使うベジェ係数 (magic number)。
/// 4/3 * tan(pi/8) ≈ 0.5522847498
const KAPPA: f32 = 0.5522847498;

/// SVG ジオメトリ要素 (rect, circle, ellipse, line, polyline, polygon, path) を
/// Vec<PathSegment> に変換する。非ジオメトリ要素の場合は None を返す。
pub fn convert_element(el: &XmlElement) -> Option<Vec<PathSegment>> {
    match el.tag.as_str() {
        "rect" => convert_rect(el),
        "circle" => convert_circle(el),
        "ellipse" => convert_ellipse(el),
        "line" => convert_line(el),
        "polyline" => convert_polyline(el),
        "polygon" => convert_polygon(el),
        "path" => convert_path(el),
        _ => None,
    }
}

// ============================================
// rect
// ============================================

/// <rect> → PathSegment 列。
///
/// - rx=0, ry=0 の場合: 単純な 4 辺の矩形
/// - rx>0 or ry>0 の場合: 角丸矩形 (CubicTo で四分円を近似)
///
/// 角丸矩形の辺と角の配置:
/// ```text
///        (x+rx, y) ─────────── (x+w-rx, y)
///       ╭                                 ╮
///  (x, y+ry)                         (x+w, y+ry)
///       │                                 │
///  (x, y+h-ry)                       (x+w, y+h-ry)
///       ╰                                 ╯
///      (x+rx, y+h) ─────────── (x+w-rx, y+h)
/// ```
fn convert_rect(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let x = attr_f32(el, "x", 0.0);
    let y = attr_f32(el, "y", 0.0);
    let w = attr_f32(el, "width", 0.0);
    let h = attr_f32(el, "height", 0.0);

    // rx/ry の解決: 片方のみ指定時はもう片方にコピー → clamp
    let raw_rx = el.attribute("rx").and_then(|v| v.parse::<f32>().ok());
    let raw_ry = el.attribute("ry").and_then(|v| v.parse::<f32>().ok());

    let (mut rx, mut ry) = match (raw_rx, raw_ry) {
        (Some(rx), Some(ry)) => (rx, ry),
        (Some(rx), None) => (rx, rx),
        (None, Some(ry)) => (ry, ry),
        (None, None) => (0.0, 0.0),
    };

    rx = rx.clamp(0.0, w / 2.0);
    ry = ry.clamp(0.0, h / 2.0);

    let mut segs = Vec::new();

    if rx == 0.0 && ry == 0.0 {
        // 角丸なし: MoveTo → 4 辺の LineTo → Close
        segs.push(PathSegment::MoveTo(x, y));
        segs.push(PathSegment::LineTo(x + w, y));
        segs.push(PathSegment::LineTo(x + w, y + h));
        segs.push(PathSegment::LineTo(x, y + h));
        segs.push(PathSegment::LineTo(x, y));
        segs.push(PathSegment::Close);
    } else {
        // 角丸あり: 右上角から時計回りに辺→角を交互に描く
        let kx = KAPPA * rx; // 角丸ベジェの x 方向オフセット
        let ky = KAPPA * ry; // 角丸ベジェの y 方向オフセット

        // 開始: 上辺の左端 (角丸の終了点)
        segs.push(PathSegment::MoveTo(x + rx, y));

        // 上辺 → 右上角
        segs.push(PathSegment::LineTo(x + w - rx, y));
        segs.push(PathSegment::CubicTo {
            ctrl1_x: x + w - rx + kx,
            ctrl1_y: y,
            ctrl2_x: x + w,
            ctrl2_y: y + ry - ky,
            end_x: x + w,
            end_y: y + ry,
        });

        // 右辺 → 右下角
        segs.push(PathSegment::LineTo(x + w, y + h - ry));
        segs.push(PathSegment::CubicTo {
            ctrl1_x: x + w,
            ctrl1_y: y + h - ry + ky,
            ctrl2_x: x + w - rx + kx,
            ctrl2_y: y + h,
            end_x: x + w - rx,
            end_y: y + h,
        });

        // 下辺 → 左下角
        segs.push(PathSegment::LineTo(x + rx, y + h));
        segs.push(PathSegment::CubicTo {
            ctrl1_x: x + rx - kx,
            ctrl1_y: y + h,
            ctrl2_x: x,
            ctrl2_y: y + h - ry + ky,
            end_x: x,
            end_y: y + h - ry,
        });

        // 左辺 → 左上角
        segs.push(PathSegment::LineTo(x, y + ry));
        segs.push(PathSegment::CubicTo {
            ctrl1_x: x,
            ctrl1_y: y + ry - ky,
            ctrl2_x: x + rx - kx,
            ctrl2_y: y,
            end_x: x + rx,
            end_y: y,
        });

        segs.push(PathSegment::Close);
    }

    Some(segs)
}

// ============================================
// circle
// ============================================

/// <circle> → PathSegment 列。
/// 4 つの CubicTo で円を近似する (四分円ごと)。
fn convert_circle(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let cx = attr_f32(el, "cx", 0.0);
    let cy = attr_f32(el, "cy", 0.0);
    let r = attr_f32(el, "r", 0.0);

    Some(ellipse_to_segments(cx, cy, r, r))
}

// ============================================
// ellipse
// ============================================

/// <ellipse> → PathSegment 列。
/// circle と同じロジックだが rx, ry を個別に使用する。
fn convert_ellipse(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let cx = attr_f32(el, "cx", 0.0);
    let cy = attr_f32(el, "cy", 0.0);
    let rx = attr_f32(el, "rx", 0.0);
    let ry = attr_f32(el, "ry", 0.0);

    Some(ellipse_to_segments(cx, cy, rx, ry))
}

/// 楕円 (cx, cy, rx, ry) を 4 つの四分円ベジェ弧で近似して PathSegment 列を返す。
///
/// ```text
///            (cx, cy-ry)          ← 上端 (12 時)
///           ╱            ╲
///  (cx-rx, cy)          (cx+rx, cy)   ← 左端 (9 時), 右端 (3 時, 開始点)
///           ╲            ╱
///            (cx, cy+ry)          ← 下端 (6 時)
/// ```
fn ellipse_to_segments(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<PathSegment> {
    let krx = KAPPA * rx;
    let kry = KAPPA * ry;

    vec![
        // 開始: 3 時の位置
        PathSegment::MoveTo(cx + rx, cy),
        // 3 時 → 6 時 (右端 → 下端)
        PathSegment::CubicTo {
            ctrl1_x: cx + rx,
            ctrl1_y: cy + kry,
            ctrl2_x: cx + krx,
            ctrl2_y: cy + ry,
            end_x: cx,
            end_y: cy + ry,
        },
        // 6 時 → 9 時 (下端 → 左端)
        PathSegment::CubicTo {
            ctrl1_x: cx - krx,
            ctrl1_y: cy + ry,
            ctrl2_x: cx - rx,
            ctrl2_y: cy + kry,
            end_x: cx - rx,
            end_y: cy,
        },
        // 9 時 → 12 時 (左端 → 上端)
        PathSegment::CubicTo {
            ctrl1_x: cx - rx,
            ctrl1_y: cy - kry,
            ctrl2_x: cx - krx,
            ctrl2_y: cy - ry,
            end_x: cx,
            end_y: cy - ry,
        },
        // 12 時 → 3 時 (上端 → 右端, 閉じる)
        PathSegment::CubicTo {
            ctrl1_x: cx + krx,
            ctrl1_y: cy - ry,
            ctrl2_x: cx + rx,
            ctrl2_y: cy - kry,
            end_x: cx + rx,
            end_y: cy,
        },
        PathSegment::Close,
    ]
}

// ============================================
// line
// ============================================

/// <line> → PathSegment 列 (MoveTo + LineTo)。
fn convert_line(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let x1 = attr_f32(el, "x1", 0.0);
    let y1 = attr_f32(el, "y1", 0.0);
    let x2 = attr_f32(el, "x2", 0.0);
    let y2 = attr_f32(el, "y2", 0.0);

    Some(vec![
        PathSegment::MoveTo(x1, y1),
        PathSegment::LineTo(x2, y2),
    ])
}

// ============================================
// polyline
// ============================================

/// <polyline> → PathSegment 列 (MoveTo + LineTo...)。
fn convert_polyline(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let points_str = el.attribute("points")?;
    let points = parse_points(points_str);
    if points.is_empty() {
        return Some(Vec::new());
    }

    let mut segs = Vec::with_capacity(points.len());
    segs.push(PathSegment::MoveTo(points[0].0, points[0].1));
    for &(px, py) in &points[1..] {
        segs.push(PathSegment::LineTo(px, py));
    }

    Some(segs)
}

// ============================================
// polygon
// ============================================

/// <polygon> → PathSegment 列 (MoveTo + LineTo... + Close)。
/// polyline と同じだが最後に Close を付加する。
fn convert_polygon(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let points_str = el.attribute("points")?;
    let points = parse_points(points_str);
    if points.is_empty() {
        return Some(Vec::new());
    }

    let mut segs = Vec::with_capacity(points.len() + 1);
    segs.push(PathSegment::MoveTo(points[0].0, points[0].1));
    for &(px, py) in &points[1..] {
        segs.push(PathSegment::LineTo(px, py));
    }
    segs.push(PathSegment::Close);

    Some(segs)
}

// ============================================
// path
// ============================================

/// <path> → PathSegment 列。d 属性を parse_path_data でパースする。
fn convert_path(el: &XmlElement) -> Option<Vec<PathSegment>> {
    let d = el.attribute("d")?;
    crate::svg::path_data::parse_path_data(d).ok()
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PathSegment;
    use crate::xml::XmlElement;

    /// テスト用の XmlElement を簡易構築するヘルパー
    fn make_element(tag: &str, attrs: &[(&str, &str)]) -> XmlElement {
        XmlElement {
            tag: tag.to_string(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children: Vec::new(),
        }
    }

    fn assert_close_f32(a: f32, b: f32, tol: f32) {
        assert!(
            (a - b).abs() <= tol,
            "Expected {} ≈ {}, diff = {}",
            a,
            b,
            (a - b).abs()
        );
    }

    // ========================================
    // rect テスト
    // ========================================

    #[test]
    fn test_rect_basic() {
        let el = make_element("rect", &[("x", "10"), ("y", "20"), ("width", "100"), ("height", "50")]);
        let segs = convert_element(&el).unwrap();

        // MoveTo + 4 LineTo + Close = 6 segments
        assert_eq!(segs.len(), 6);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(110.0, 20.0));
        assert_eq!(segs[2], PathSegment::LineTo(110.0, 70.0));
        assert_eq!(segs[3], PathSegment::LineTo(10.0, 70.0));
        assert_eq!(segs[4], PathSegment::LineTo(10.0, 20.0));
        assert_eq!(segs[5], PathSegment::Close);
    }

    #[test]
    fn test_rect_with_rounded_corners() {
        let el = make_element(
            "rect",
            &[("x", "0"), ("y", "0"), ("width", "100"), ("height", "50"), ("rx", "10"), ("ry", "5")],
        );
        let segs = convert_element(&el).unwrap();

        // 角丸矩形: MoveTo + (LineTo + CubicTo) × 4 + Close = 1 + 8 + 1 = 10
        assert_eq!(segs.len(), 10);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 0.0)); // (x+rx, y)
        assert_eq!(segs[1], PathSegment::LineTo(90.0, 0.0)); // 上辺 → (x+w-rx, y)
        // segs[2]: 右上角の CubicTo
        match segs[2] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, 100.0, 0.01);
                assert_close_f32(end_y, 5.0, 0.01); // y+ry
            }
            _ => panic!("Expected CubicTo for top-right corner"),
        }
        assert_eq!(segs[9], PathSegment::Close);
    }

    #[test]
    fn test_rect_rx_only() {
        // rx のみ指定 → ry = rx
        let el = make_element("rect", &[("width", "100"), ("height", "50"), ("rx", "10")]);
        let segs = convert_element(&el).unwrap();
        // 角丸ありなので 10 セグメント
        assert_eq!(segs.len(), 10);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 0.0));
    }

    #[test]
    fn test_rect_ry_only() {
        // ry のみ指定 → rx = ry
        let el = make_element("rect", &[("width", "100"), ("height", "50"), ("ry", "8")]);
        let segs = convert_element(&el).unwrap();
        assert_eq!(segs.len(), 10);
        assert_eq!(segs[0], PathSegment::MoveTo(8.0, 0.0)); // rx = ry = 8
    }

    #[test]
    fn test_rect_rx_clamped() {
        // rx > width/2 → clamp
        let el = make_element("rect", &[("width", "20"), ("height", "50"), ("rx", "30")]);
        let segs = convert_element(&el).unwrap();
        // rx は 10 (= 20/2) にクランプされる
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 0.0));
    }

    // ========================================
    // circle テスト
    // ========================================

    #[test]
    fn test_circle() {
        let el = make_element("circle", &[("cx", "50"), ("cy", "50"), ("r", "25")]);
        let segs = convert_element(&el).unwrap();

        // MoveTo + 4 CubicTo + Close = 6
        assert_eq!(segs.len(), 6);
        assert_eq!(segs[0], PathSegment::MoveTo(75.0, 50.0)); // cx+r, cy
        // 4 つの CubicTo を検証
        for i in 1..=4 {
            assert!(matches!(segs[i], PathSegment::CubicTo { .. }), "segs[{}] should be CubicTo", i);
        }
        assert_eq!(segs[5], PathSegment::Close);

        // 最後の CubicTo の終点が開始点に戻ること
        match segs[4] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, 75.0, 0.01);
                assert_close_f32(end_y, 50.0, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    // ========================================
    // ellipse テスト
    // ========================================

    #[test]
    fn test_ellipse() {
        let el = make_element("ellipse", &[("cx", "100"), ("cy", "50"), ("rx", "80"), ("ry", "30")]);
        let segs = convert_element(&el).unwrap();

        // MoveTo + 4 CubicTo + Close = 6
        assert_eq!(segs.len(), 6);
        assert_eq!(segs[0], PathSegment::MoveTo(180.0, 50.0)); // cx+rx, cy

        // 最初の CubicTo: 右端 → 下端
        match segs[1] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, 100.0, 0.01); // cx
                assert_close_f32(end_y, 80.0, 0.01);  // cy+ry
            }
            _ => panic!("Expected CubicTo"),
        }

        assert_eq!(segs[5], PathSegment::Close);
    }

    // ========================================
    // line テスト
    // ========================================

    #[test]
    fn test_line() {
        let el = make_element("line", &[("x1", "10"), ("y1", "20"), ("x2", "30"), ("y2", "40")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
    }

    #[test]
    fn test_line_default_values() {
        // 属性省略時は 0.0
        let el = make_element("line", &[("x2", "50"), ("y2", "60")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs[0], PathSegment::MoveTo(0.0, 0.0));
        assert_eq!(segs[1], PathSegment::LineTo(50.0, 60.0));
    }

    // ========================================
    // polyline テスト
    // ========================================

    #[test]
    fn test_polyline() {
        let el = make_element("polyline", &[("points", "10,20 30,40 50,60")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
        assert_eq!(segs[2], PathSegment::LineTo(50.0, 60.0));
    }

    #[test]
    fn test_polyline_space_separated() {
        let el = make_element("polyline", &[("points", "1 2 3 4 5 6")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(1.0, 2.0));
        assert_eq!(segs[1], PathSegment::LineTo(3.0, 4.0));
        assert_eq!(segs[2], PathSegment::LineTo(5.0, 6.0));
    }

    // ========================================
    // polygon テスト
    // ========================================

    #[test]
    fn test_polygon() {
        let el = make_element("polygon", &[("points", "10,20 30,40 50,60")]);
        let segs = convert_element(&el).unwrap();

        // MoveTo + 2 LineTo + Close = 4
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
        assert_eq!(segs[2], PathSegment::LineTo(50.0, 60.0));
        assert_eq!(segs[3], PathSegment::Close);
    }

    // ========================================
    // path テスト
    // ========================================

    #[test]
    fn test_path_with_d() {
        let el = make_element("path", &[("d", "M 0 0 L 10 20 Z")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(0.0, 0.0));
        assert_eq!(segs[1], PathSegment::LineTo(10.0, 20.0));
        assert_eq!(segs[2], PathSegment::Close);
    }

    #[test]
    fn test_path_no_d_returns_none() {
        let el = make_element("path", &[("fill", "red")]);
        assert!(convert_element(&el).is_none());
    }

    // ========================================
    // 不明タグ
    // ========================================

    #[test]
    fn test_unknown_tag_returns_none() {
        let el = make_element("g", &[]);
        assert!(convert_element(&el).is_none());
    }

    #[test]
    fn test_unknown_tag_text_returns_none() {
        let el = make_element("text", &[("x", "0"), ("y", "0")]);
        assert!(convert_element(&el).is_none());
    }

    // ========================================
    // checkout.svg のテストデータ
    // ========================================

    #[test]
    fn test_checkout_rect() {
        // <rect x="-8" y="-9" width="16" height="18" rx="2" ...>
        let el = make_element(
            "rect",
            &[("x", "-8"), ("y", "-9"), ("width", "16"), ("height", "18"), ("rx", "2")],
        );
        let segs = convert_element(&el).unwrap();

        // rx=2, ry=2 (rx のみ指定なので ry=rx)
        // 角丸ありなので 10 セグメント
        assert_eq!(segs.len(), 10);

        // 開始点: (x+rx, y) = (-8+2, -9) = (-6, -9)
        assert_eq!(segs[0], PathSegment::MoveTo(-6.0, -9.0));

        // 上辺の終了点: (x+w-rx, y) = (-8+16-2, -9) = (6, -9)
        assert_eq!(segs[1], PathSegment::LineTo(6.0, -9.0));

        // 右上角の CubicTo 終点: (x+w, y+ry) = (8, -7)
        match segs[2] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, 8.0, 0.01);
                assert_close_f32(end_y, -7.0, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }

        assert_eq!(segs[9], PathSegment::Close);
    }

    #[test]
    fn test_checkout_circle() {
        // <circle cx="0" cy="0" r="4.5" ...>
        let el = make_element("circle", &[("cx", "0"), ("cy", "0"), ("r", "4.5")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 6);
        assert_eq!(segs[0], PathSegment::MoveTo(4.5, 0.0));

        // 最初の CubicTo: (cx+r, cy) → (cx, cy+r) = (4.5, 0) → (0, 4.5)
        let kr = KAPPA * 4.5;
        match segs[1] {
            PathSegment::CubicTo {
                ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, end_x, end_y,
            } => {
                assert_close_f32(ctrl1_x, 4.5, 0.01);
                assert_close_f32(ctrl1_y, kr, 0.01);
                assert_close_f32(ctrl2_x, kr, 0.01);
                assert_close_f32(ctrl2_y, 4.5, 0.01);
                assert_close_f32(end_x, 0.0, 0.01);
                assert_close_f32(end_y, 4.5, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }

        assert_eq!(segs[5], PathSegment::Close);
    }

    #[test]
    fn test_checkout_ellipse() {
        // <ellipse cx="0" cy="28" rx="26" ry="3.5" ...>
        let el = make_element("ellipse", &[("cx", "0"), ("cy", "28"), ("rx", "26"), ("ry", "3.5")]);
        let segs = convert_element(&el).unwrap();

        assert_eq!(segs.len(), 6);
        assert_eq!(segs[0], PathSegment::MoveTo(26.0, 28.0)); // cx+rx, cy

        // 最初の CubicTo 終点: (cx, cy+ry) = (0, 31.5)
        match segs[1] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, 0.0, 0.01);
                assert_close_f32(end_y, 31.5, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }

        // 2 番目の CubicTo 終点: (cx-rx, cy) = (-26, 28)
        match segs[2] {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(end_x, -26.0, 0.01);
                assert_close_f32(end_y, 28.0, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }

        assert_eq!(segs[5], PathSegment::Close);
    }

    #[test]
    fn test_checkout_polygon() {
        // polygon points="-18,-15 -12,10 15,10 25,-15"
        let el = make_element("polygon", &[("points", "-18,-15 -12,10 15,10 25,-15")]);
        let segs = convert_element(&el).unwrap();

        // MoveTo + 3 LineTo + Close = 5
        assert_eq!(segs.len(), 5);
        assert_eq!(segs[0], PathSegment::MoveTo(-18.0, -15.0));
        assert_eq!(segs[1], PathSegment::LineTo(-12.0, 10.0));
        assert_eq!(segs[2], PathSegment::LineTo(15.0, 10.0));
        assert_eq!(segs[3], PathSegment::LineTo(25.0, -15.0));
        assert_eq!(segs[4], PathSegment::Close);
    }

    // ========================================
    // parse_points ヘルパーのテスト
    // ========================================

    #[test]
    fn test_parse_points_comma_separated() {
        let pts = parse_points("1,2 3,4 5,6");
        assert_eq!(pts, vec![(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)]);
    }

    #[test]
    fn test_parse_points_space_separated() {
        let pts = parse_points("1 2 3 4");
        assert_eq!(pts, vec![(1.0, 2.0), (3.0, 4.0)]);
    }

    #[test]
    fn test_parse_points_empty() {
        let pts = parse_points("");
        assert!(pts.is_empty());
    }

    #[test]
    fn test_parse_points_odd_count() {
        // 奇数個の場合、末尾の余りは無視
        let pts = parse_points("1,2 3");
        assert_eq!(pts, vec![(1.0, 2.0)]);
    }

    #[test]
    fn test_parse_points_negative() {
        let pts = parse_points("-1,-2 -3,-4");
        assert_eq!(pts, vec![(-1.0, -2.0), (-3.0, -4.0)]);
    }
}

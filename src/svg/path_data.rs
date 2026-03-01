use crate::types::PathSegment;

/// SVG path `d` 属性文字列を Vec<PathSegment> にパースする。
/// 全コマンドを MoveTo/LineTo/CubicTo/Close の 4 種に正規化する。
pub fn parse_path_data(d: &str) -> Result<Vec<PathSegment>, String> {
    let mut tokenizer = PathTokenizer::new(d);
    let mut segments = Vec::new();

    // 現在のカーソル位置
    let mut cx: f32 = 0.0;
    let mut cy: f32 = 0.0;
    // 最後の MoveTo の位置 (Close で戻る先)
    let mut start_x: f32 = 0.0;
    let mut start_y: f32 = 0.0;
    // 直前の制御点 (S/T コマンドの反射用)
    let mut last_ctrl2_x: f32 = 0.0;
    let mut last_ctrl2_y: f32 = 0.0;
    let mut last_quad_ctrl_x: f32 = 0.0;
    let mut last_quad_ctrl_y: f32 = 0.0;
    // 直前のコマンド種別 (S/T コマンドの反射判定用)
    let mut last_cmd: char = ' ';

    while let Some(cmd) = tokenizer.next_command() {
        match cmd {
            // ============================================
            // MoveTo: M (absolute) / m (relative)
            // ============================================
            'M' | 'm' => {
                let relative = cmd == 'm';
                let mut first = true;
                while tokenizer.has_number() {
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (abs_x, abs_y) = if relative { (cx + x, cy + y) } else { (x, y) };

                    if first {
                        segments.push(PathSegment::MoveTo(abs_x, abs_y));
                        start_x = abs_x;
                        start_y = abs_y;
                        first = false;
                    } else {
                        // M の後続座標は暗黙の L (m の場合は暗黙の l)
                        segments.push(PathSegment::LineTo(abs_x, abs_y));
                    }
                    cx = abs_x;
                    cy = abs_y;
                }
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // LineTo: L (absolute) / l (relative)
            // ============================================
            'L' | 'l' => {
                let relative = cmd == 'l';
                while tokenizer.has_number() {
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (abs_x, abs_y) = if relative { (cx + x, cy + y) } else { (x, y) };
                    segments.push(PathSegment::LineTo(abs_x, abs_y));
                    cx = abs_x;
                    cy = abs_y;
                }
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // Horizontal LineTo: H (absolute) / h (relative)
            // ============================================
            'H' | 'h' => {
                let relative = cmd == 'h';
                while tokenizer.has_number() {
                    let x = tokenizer.next_number()?;
                    let abs_x = if relative { cx + x } else { x };
                    segments.push(PathSegment::LineTo(abs_x, cy));
                    cx = abs_x;
                }
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // Vertical LineTo: V (absolute) / v (relative)
            // ============================================
            'V' | 'v' => {
                let relative = cmd == 'v';
                while tokenizer.has_number() {
                    let y = tokenizer.next_number()?;
                    let abs_y = if relative { cy + y } else { y };
                    segments.push(PathSegment::LineTo(cx, abs_y));
                    cy = abs_y;
                }
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // CubicTo: C (absolute) / c (relative)
            // ============================================
            'C' | 'c' => {
                let relative = cmd == 'c';
                while tokenizer.has_number() {
                    let x1 = tokenizer.next_number()?;
                    let y1 = tokenizer.next_number()?;
                    let x2 = tokenizer.next_number()?;
                    let y2 = tokenizer.next_number()?;
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
                    let abs_x1 = ox + x1;
                    let abs_y1 = oy + y1;
                    let abs_x2 = ox + x2;
                    let abs_y2 = oy + y2;
                    let abs_x = ox + x;
                    let abs_y = oy + y;
                    segments.push(PathSegment::CubicTo {
                        ctrl1_x: abs_x1,
                        ctrl1_y: abs_y1,
                        ctrl2_x: abs_x2,
                        ctrl2_y: abs_y2,
                        end_x: abs_x,
                        end_y: abs_y,
                    });
                    last_ctrl2_x = abs_x2;
                    last_ctrl2_y = abs_y2;
                    cx = abs_x;
                    cy = abs_y;
                }
                last_cmd = cmd;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // Smooth CubicTo: S (absolute) / s (relative)
            // ctrl1 は前回の ctrl2 の反射。前回が C/c/S/s でなければ ctrl1 = current pos
            // ============================================
            'S' | 's' => {
                let relative = cmd == 's';
                while tokenizer.has_number() {
                    let x2 = tokenizer.next_number()?;
                    let y2 = tokenizer.next_number()?;
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
                    let abs_x2 = ox + x2;
                    let abs_y2 = oy + y2;
                    let abs_x = ox + x;
                    let abs_y = oy + y;

                    // ctrl1: 前回が C/c/S/s なら ctrl2 の反射、そうでなければ現在位置
                    let (c1x, c1y) =
                        if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                            (2.0 * cx - last_ctrl2_x, 2.0 * cy - last_ctrl2_y)
                        } else {
                            (cx, cy)
                        };

                    segments.push(PathSegment::CubicTo {
                        ctrl1_x: c1x,
                        ctrl1_y: c1y,
                        ctrl2_x: abs_x2,
                        ctrl2_y: abs_y2,
                        end_x: abs_x,
                        end_y: abs_y,
                    });
                    last_ctrl2_x = abs_x2;
                    last_ctrl2_y = abs_y2;
                    cx = abs_x;
                    cy = abs_y;
                    last_cmd = cmd;
                }
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // Quadratic Bezier: Q (absolute) / q (relative)
            // 二次→三次ベジェ昇格:
            //   C1 = P0 + 2/3 * (Q - P0)
            //   C2 = P1 + 2/3 * (Q - P1)
            // ============================================
            'Q' | 'q' => {
                let relative = cmd == 'q';
                while tokenizer.has_number() {
                    let qx = tokenizer.next_number()?;
                    let qy = tokenizer.next_number()?;
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
                    let abs_qx = ox + qx;
                    let abs_qy = oy + qy;
                    let abs_x = ox + x;
                    let abs_y = oy + y;

                    let (c1x, c1y, c2x, c2y) =
                        quad_to_cubic(cx, cy, abs_qx, abs_qy, abs_x, abs_y);
                    segments.push(PathSegment::CubicTo {
                        ctrl1_x: c1x,
                        ctrl1_y: c1y,
                        ctrl2_x: c2x,
                        ctrl2_y: c2y,
                        end_x: abs_x,
                        end_y: abs_y,
                    });
                    last_quad_ctrl_x = abs_qx;
                    last_quad_ctrl_y = abs_qy;
                    last_ctrl2_x = c2x;
                    last_ctrl2_y = c2y;
                    cx = abs_x;
                    cy = abs_y;
                    last_cmd = cmd;
                }
            }

            // ============================================
            // Smooth Quadratic: T (absolute) / t (relative)
            // Q の制御点を前回の Q/T の反射として計算
            // ============================================
            'T' | 't' => {
                let relative = cmd == 't';
                while tokenizer.has_number() {
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
                    let abs_x = ox + x;
                    let abs_y = oy + y;

                    // 前回が Q/q/T/t なら Q 制御点の反射、そうでなければ現在位置
                    let (qx, qy) =
                        if matches!(last_cmd, 'Q' | 'q' | 'T' | 't') {
                            (2.0 * cx - last_quad_ctrl_x, 2.0 * cy - last_quad_ctrl_y)
                        } else {
                            (cx, cy)
                        };

                    let (c1x, c1y, c2x, c2y) = quad_to_cubic(cx, cy, qx, qy, abs_x, abs_y);
                    segments.push(PathSegment::CubicTo {
                        ctrl1_x: c1x,
                        ctrl1_y: c1y,
                        ctrl2_x: c2x,
                        ctrl2_y: c2y,
                        end_x: abs_x,
                        end_y: abs_y,
                    });
                    last_quad_ctrl_x = qx;
                    last_quad_ctrl_y = qy;
                    last_ctrl2_x = c2x;
                    last_ctrl2_y = c2y;
                    cx = abs_x;
                    cy = abs_y;
                    last_cmd = cmd;
                }
            }

            // ============================================
            // Elliptical Arc: A (absolute) / a (relative)
            // 楕円弧 → 三次ベジェ近似
            // ============================================
            'A' | 'a' => {
                let relative = cmd == 'a';
                while tokenizer.has_number() {
                    let rx = tokenizer.next_number()?;
                    let ry = tokenizer.next_number()?;
                    let x_rotation = tokenizer.next_number()?;
                    let large_arc = tokenizer.next_flag()?;
                    let sweep = tokenizer.next_flag()?;
                    let x = tokenizer.next_number()?;
                    let y = tokenizer.next_number()?;
                    let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
                    let abs_x = ox + x;
                    let abs_y = oy + y;

                    arc_to_cubics(
                        cx,
                        cy,
                        rx,
                        ry,
                        x_rotation,
                        large_arc,
                        sweep,
                        abs_x,
                        abs_y,
                        &mut segments,
                    );
                    cx = abs_x;
                    cy = abs_y;
                }
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            // ============================================
            // Close: Z / z
            // ============================================
            'Z' | 'z' => {
                segments.push(PathSegment::Close);
                cx = start_x;
                cy = start_y;
                last_cmd = cmd;
                last_ctrl2_x = cx;
                last_ctrl2_y = cy;
                last_quad_ctrl_x = cx;
                last_quad_ctrl_y = cy;
            }

            _ => return Err(format!("Unknown path command: '{}'", cmd)),
        }
    }

    Ok(segments)
}

// ============================================
// 二次ベジェ → 三次ベジェ昇格
// ============================================

/// 二次ベジェ (P0, Q, P1) を三次ベジェの制御点 (C1, C2) に変換する。
///   C1 = P0 + 2/3 * (Q - P0)
///   C2 = P1 + 2/3 * (Q - P1)
fn quad_to_cubic(
    p0x: f32, p0y: f32,
    qx: f32, qy: f32,
    p1x: f32, p1y: f32,
) -> (f32, f32, f32, f32) {
    let c1x = p0x + 2.0 / 3.0 * (qx - p0x);
    let c1y = p0y + 2.0 / 3.0 * (qy - p0y);
    let c2x = p1x + 2.0 / 3.0 * (qx - p1x);
    let c2y = p1y + 2.0 / 3.0 * (qy - p1y);
    (c1x, c1y, c2x, c2y)
}

// ============================================
// 楕円弧 → 三次ベジェ近似
// SVG spec §F.6.5 / §F.6.6 のアルゴリズム
// ============================================

/// 楕円弧を三次ベジェ曲線群に変換して segments に追加する。
fn arc_to_cubics(
    x1: f32, y1: f32,
    mut rx: f32, mut ry: f32,
    x_rotation_deg: f32,
    large_arc: bool, sweep: bool,
    x2: f32, y2: f32,
    segments: &mut Vec<PathSegment>,
) {
    // 始点と終点が同じなら何もしない
    if (x1 - x2).abs() < 1e-10 && (y1 - y2).abs() < 1e-10 {
        return;
    }

    // rx, ry が 0 なら直線
    if rx.abs() < 1e-10 || ry.abs() < 1e-10 {
        segments.push(PathSegment::LineTo(x2, y2));
        return;
    }

    rx = rx.abs();
    ry = ry.abs();

    let phi = x_rotation_deg.to_radians();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    // §F.6.5.1: 座標変換
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    // §F.6.6.2: 半径の補正
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let mut rx2 = rx * rx;
    let mut ry2 = ry * ry;
    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let sqrt_lambda = lambda.sqrt();
        rx *= sqrt_lambda;
        ry *= sqrt_lambda;
        rx2 = rx * rx;
        ry2 = ry * ry;
    }

    // §F.6.5.2: 中心座標の計算
    let num = (rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2).max(0.0);
    let den = rx2 * y1p2 + ry2 * x1p2;
    let sq = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let cxp = sign * sq * (rx * y1p / ry);
    let cyp = sign * sq * -(ry * x1p / rx);

    // §F.6.5.3: 中心座標を元の座標系に戻す
    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;

    // §F.6.5.5: 角度の計算
    let theta1 = angle_between(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle_between(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    // sweep フラグに応じて角度範囲を調整
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f32::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f32::consts::TAU;
    }

    // 弧を 90° 以下の区間に分割
    let n_segs = (dtheta.abs() / (std::f32::consts::FRAC_PI_2 + 0.001)).ceil() as usize;
    let n_segs = n_segs.max(1);
    let d_per_seg = dtheta / n_segs as f32;

    for i in 0..n_segs {
        let t1 = theta1 + d_per_seg * i as f32;
        let t2 = theta1 + d_per_seg * (i + 1) as f32;
        arc_segment_to_cubic(cx, cy, rx, ry, cos_phi, sin_phi, t1, t2, segments);
    }
}

/// 1 つの弧区間 (≤90°) を三次ベジェに変換する。
/// α = 4/3 * tan(θ/4) で制御点を計算する。
fn arc_segment_to_cubic(
    cx: f32, cy: f32,
    rx: f32, ry: f32,
    cos_phi: f32, sin_phi: f32,
    theta1: f32, theta2: f32,
    segments: &mut Vec<PathSegment>,
) {
    let half_dtheta = (theta2 - theta1) / 2.0;
    let alpha = (4.0 / 3.0) * (half_dtheta / 2.0).tan();

    let cos1 = theta1.cos();
    let sin1 = theta1.sin();
    let cos2 = theta2.cos();
    let sin2 = theta2.sin();

    // 楕円上の点 (回転前)
    let ex1 = rx * cos1;
    let ey1 = ry * sin1;
    let ex2 = rx * cos2;
    let ey2 = ry * sin2;

    // 制御点 (回転前)
    let cp1x = ex1 - alpha * rx * sin1;
    let cp1y = ey1 + alpha * ry * cos1;
    let cp2x = ex2 + alpha * rx * sin2;
    let cp2y = ey2 - alpha * ry * cos2;

    // 回転 + 平行移動を適用
    let c1x = cos_phi * cp1x - sin_phi * cp1y + cx;
    let c1y = sin_phi * cp1x + cos_phi * cp1y + cy;
    let c2x = cos_phi * cp2x - sin_phi * cp2y + cx;
    let c2y = sin_phi * cp2x + cos_phi * cp2y + cy;
    let end_x = cos_phi * ex2 - sin_phi * ey2 + cx;
    let end_y = sin_phi * ex2 + cos_phi * ey2 + cy;

    segments.push(PathSegment::CubicTo {
        ctrl1_x: c1x,
        ctrl1_y: c1y,
        ctrl2_x: c2x,
        ctrl2_y: c2y,
        end_x,
        end_y,
    });
}

/// 2D ベクトル (ux, uy) と (vx, vy) のなす角を返す (ラジアン, 符号付き)。
fn angle_between(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    let dot = ux * vx + uy * vy;
    let len_u = (ux * ux + uy * uy).sqrt();
    let len_v = (vx * vx + vy * vy).sqrt();
    let cos_val = (dot / (len_u * len_v)).clamp(-1.0, 1.0);
    let angle = cos_val.acos();
    let cross = ux * vy - uy * vx;
    if cross < 0.0 { -angle } else { angle }
}

// ============================================
// PathTokenizer: d 属性文字列のトークナイザー
// ============================================

/// SVG path `d` 属性文字列から数値とコマンドを順次読み取るトークナイザー。
///
/// 数値の区切り:
/// - スペース / カンマ / マイナス記号 (例: "10-20" → 10, -20)
/// - 小数点が連続する場合も分割 (例: "1.5.7" → 1.5, 0.7)
struct PathTokenizer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> PathTokenizer<'a> {
    fn new(d: &'a str) -> Self {
        Self {
            data: d.as_bytes(),
            pos: 0,
        }
    }

    fn skip_whitespace_and_commas(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' || b == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// 次のコマンド文字を返す。数値やカンマはスキップする。
    fn next_command(&mut self) -> Option<char> {
        self.skip_whitespace_and_commas();
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if b.is_ascii_alphabetic() {
                self.pos += 1;
                return Some(b as char);
            }
            // 数値の先頭はコマンドではない
            if b == b'-' || b == b'+' || b == b'.' || b.is_ascii_digit() {
                return None;
            }
            // その他の空白やカンマはスキップ
            self.pos += 1;
        }
        None
    }

    /// 数値が続いているか判定する。
    fn has_number(&mut self) -> bool {
        self.skip_whitespace_and_commas();
        if self.pos >= self.data.len() {
            return false;
        }
        let b = self.data[self.pos];
        b == b'-' || b == b'+' || b == b'.' || b.is_ascii_digit()
    }

    /// 次の数値を読み取る。
    fn next_number(&mut self) -> Result<f32, String> {
        self.skip_whitespace_and_commas();
        if self.pos >= self.data.len() {
            return Err("Expected number, got end of input".to_string());
        }

        let start = self.pos;
        let mut has_dot = false;
        let mut has_exp = false;

        // 符号
        if self.pos < self.data.len()
            && (self.data[self.pos] == b'-' || self.data[self.pos] == b'+')
        {
            self.pos += 1;
        }

        // 数字と小数点
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if b.is_ascii_digit() {
                self.pos += 1;
            } else if b == b'.' && !has_dot && !has_exp {
                has_dot = true;
                self.pos += 1;
            } else if (b == b'e' || b == b'E') && !has_exp {
                has_exp = true;
                self.pos += 1;
                // 指数部の符号
                if self.pos < self.data.len()
                    && (self.data[self.pos] == b'-' || self.data[self.pos] == b'+')
                {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }

        if start == self.pos {
            return Err(format!(
                "Expected number at position {}",
                self.pos
            ));
        }

        let s = std::str::from_utf8(&self.data[start..self.pos])
            .map_err(|e| format!("Invalid UTF-8 in number: {}", e))?;
        s.parse::<f32>()
            .map_err(|e| format!("Failed to parse number '{}': {}", s, e))
    }

    /// arc コマンド用のフラグ (0 or 1) を読み取る。
    fn next_flag(&mut self) -> Result<bool, String> {
        self.skip_whitespace_and_commas();
        if self.pos >= self.data.len() {
            return Err("Expected flag, got end of input".to_string());
        }
        let b = self.data[self.pos];
        match b {
            b'0' => {
                self.pos += 1;
                Ok(false)
            }
            b'1' => {
                self.pos += 1;
                Ok(true)
            }
            _ => Err(format!("Expected flag (0 or 1), got '{}'", b as char)),
        }
    }
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PathSegment;

    fn assert_close_f32(a: f32, b: f32, tol: f32) {
        assert!(
            (a - b).abs() <= tol,
            "Expected {} ≈ {}, diff = {}",
            a, b, (a - b).abs()
        );
    }

    #[test]
    fn test_simple_move_line() {
        let segs = parse_path_data("M 10 20 L 30 40").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
    }

    #[test]
    fn test_checkout_svg_path1() {
        // "M -8 -3 L 0 -1 L 8 -3"
        let segs = parse_path_data("M -8 -3 L 0 -1 L 8 -3").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(-8.0, -3.0));
        assert_eq!(segs[1], PathSegment::LineTo(0.0, -1.0));
        assert_eq!(segs[2], PathSegment::LineTo(8.0, -3.0));
    }

    #[test]
    fn test_checkout_svg_path2() {
        // "M -10 2 L -3 9 L 12 -6"
        let segs = parse_path_data("M -10 2 L -3 9 L 12 -6").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(-10.0, 2.0));
        assert_eq!(segs[1], PathSegment::LineTo(-3.0, 9.0));
        assert_eq!(segs[2], PathSegment::LineTo(12.0, -6.0));
    }

    #[test]
    fn test_move_line_close() {
        // "M0 0L10 10Z"
        let segs = parse_path_data("M0 0L10 10Z").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(0.0, 0.0));
        assert_eq!(segs[1], PathSegment::LineTo(10.0, 10.0));
        assert_eq!(segs[2], PathSegment::Close);
    }

    #[test]
    fn test_horizontal_line() {
        let segs = parse_path_data("M 0 0 H 50").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1], PathSegment::LineTo(50.0, 0.0));
    }

    #[test]
    fn test_vertical_line() {
        let segs = parse_path_data("M 0 0 V 50").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1], PathSegment::LineTo(0.0, 50.0));
    }

    #[test]
    fn test_relative_move_line() {
        let segs = parse_path_data("m 10 20 l 30 40").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(40.0, 60.0)); // 10+30, 20+40
    }

    #[test]
    fn test_relative_horizontal_vertical() {
        let segs = parse_path_data("M 10 10 h 20 v 30").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 10.0)); // 10+20, 10
        assert_eq!(segs[2], PathSegment::LineTo(30.0, 40.0)); // 30, 10+30
    }

    #[test]
    fn test_implicit_lineto_after_move() {
        // M の後続座標は暗黙の L
        let segs = parse_path_data("M 10 20 30 40 50 60").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
        assert_eq!(segs[2], PathSegment::LineTo(50.0, 60.0));
    }

    #[test]
    fn test_implicit_lineto_after_relative_move() {
        // m の後続座標は暗黙の l
        let segs = parse_path_data("m 10 20 30 40").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(40.0, 60.0)); // 10+30, 20+40
    }

    #[test]
    fn test_implicit_repetition_lineto() {
        // L 10 20 30 40 = L 10 20 L 30 40
        let segs = parse_path_data("M 0 0 L 10 20 30 40").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1], PathSegment::LineTo(10.0, 20.0));
        assert_eq!(segs[2], PathSegment::LineTo(30.0, 40.0));
    }

    #[test]
    fn test_minus_sign_separator() {
        // "10-20" = "10, -20"
        let segs = parse_path_data("M10-20L30-40").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, -20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, -40.0));
    }

    #[test]
    fn test_cubic_bezier() {
        let segs = parse_path_data("M 0 0 C 10 20 30 40 50 60").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(
            segs[1],
            PathSegment::CubicTo {
                ctrl1_x: 10.0,
                ctrl1_y: 20.0,
                ctrl2_x: 30.0,
                ctrl2_y: 40.0,
                end_x: 50.0,
                end_y: 60.0,
            }
        );
    }

    #[test]
    fn test_relative_cubic_bezier() {
        let segs = parse_path_data("M 10 10 c 10 20 30 40 50 60").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(
            segs[1],
            PathSegment::CubicTo {
                ctrl1_x: 20.0,  // 10+10
                ctrl1_y: 30.0,  // 10+20
                ctrl2_x: 40.0,  // 10+30
                ctrl2_y: 50.0,  // 10+40
                end_x: 60.0,    // 10+50
                end_y: 70.0,    // 10+60
            }
        );
    }

    #[test]
    fn test_smooth_cubic() {
        // S: ctrl1 は前回の ctrl2 の反射
        let segs = parse_path_data("M 0 0 C 10 20 30 40 50 60 S 80 90 100 110").unwrap();
        assert_eq!(segs.len(), 3);
        // ctrl1 = 2*(50,60) - (30,40) = (70, 80)
        assert_eq!(
            segs[2],
            PathSegment::CubicTo {
                ctrl1_x: 70.0,
                ctrl1_y: 80.0,
                ctrl2_x: 80.0,
                ctrl2_y: 90.0,
                end_x: 100.0,
                end_y: 110.0,
            }
        );
    }

    #[test]
    fn test_smooth_cubic_without_prior_c() {
        // S without prior C/S: ctrl1 = current pos
        let segs = parse_path_data("M 10 20 S 30 40 50 60").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(
            segs[1],
            PathSegment::CubicTo {
                ctrl1_x: 10.0,
                ctrl1_y: 20.0,
                ctrl2_x: 30.0,
                ctrl2_y: 40.0,
                end_x: 50.0,
                end_y: 60.0,
            }
        );
    }

    #[test]
    fn test_quadratic_to_cubic() {
        let segs = parse_path_data("M 0 0 Q 50 100 100 0").unwrap();
        assert_eq!(segs.len(), 2);
        match segs[1] {
            PathSegment::CubicTo {
                ctrl1_x,
                ctrl1_y,
                ctrl2_x,
                ctrl2_y,
                end_x,
                end_y,
            } => {
                // C1 = P0 + 2/3*(Q-P0) = (0,0) + 2/3*(50,100) = (33.33, 66.67)
                assert_close_f32(ctrl1_x, 33.333, 0.01);
                assert_close_f32(ctrl1_y, 66.667, 0.01);
                // C2 = P1 + 2/3*(Q-P1) = (100,0) + 2/3*(-50,100) = (66.67, 66.67)
                assert_close_f32(ctrl2_x, 66.667, 0.01);
                assert_close_f32(ctrl2_y, 66.667, 0.01);
                assert_close_f32(end_x, 100.0, 0.01);
                assert_close_f32(end_y, 0.0, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_smooth_quadratic() {
        let segs = parse_path_data("M 0 0 Q 25 50 50 0 T 100 0").unwrap();
        assert_eq!(segs.len(), 3);
        // T: Q reflected = 2*(50,0) - (25,50) = (75, -50)
        match segs[2] {
            PathSegment::CubicTo {
                ctrl1_x,
                ctrl1_y,
                ctrl2_x,
                ctrl2_y,
                end_x,
                end_y,
            } => {
                // C1 = (50,0) + 2/3*(75,-50 - 50,0) = (50,0) + 2/3*(25,-50) = (66.67, -33.33)
                assert_close_f32(ctrl1_x, 66.667, 0.01);
                assert_close_f32(ctrl1_y, -33.333, 0.01);
                // C2 = (100,0) + 2/3*(75,-50 - 100,0) = (100,0) + 2/3*(-25,-50) = (83.33, -33.33)
                assert_close_f32(ctrl2_x, 83.333, 0.01);
                assert_close_f32(ctrl2_y, -33.333, 0.01);
                assert_close_f32(end_x, 100.0, 0.01);
                assert_close_f32(end_y, 0.0, 0.01);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_arc_to_cubic() {
        // 単純な半円弧
        let segs = parse_path_data("M 0 0 A 50 50 0 0 1 100 0").unwrap();
        // MoveTo + N個の CubicTo
        assert_eq!(segs[0], PathSegment::MoveTo(0.0, 0.0));
        assert!(segs.len() >= 2); // 少なくとも1つの CubicTo
        // 最後の CubicTo の終点が (100, 0) に近い
        match segs.last().unwrap() {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(*end_x, 100.0, 0.1);
                assert_close_f32(*end_y, 0.0, 0.1);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_arc_degenerate_zero_radius() {
        // rx=0 or ry=0 なら直線
        let segs = parse_path_data("M 0 0 A 0 50 0 0 1 100 0").unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1], PathSegment::LineTo(100.0, 0.0));
    }

    #[test]
    fn test_arc_same_start_end() {
        // 始点と終点が同じなら何も追加しない
        let segs = parse_path_data("M 50 50 A 25 25 0 0 1 50 50").unwrap();
        assert_eq!(segs.len(), 1); // MoveTo only
    }

    #[test]
    fn test_close_resets_position() {
        let segs = parse_path_data("M 10 20 L 30 40 Z L 50 60").unwrap();
        assert_eq!(segs.len(), 4);
        // Z 後のカーソルは (10,20) に戻る → L 50 60 はそこから
        assert_eq!(segs[3], PathSegment::LineTo(50.0, 60.0));
    }

    #[test]
    fn test_comma_separated() {
        let segs = parse_path_data("M10,20L30,40").unwrap();
        assert_eq!(segs[0], PathSegment::MoveTo(10.0, 20.0));
        assert_eq!(segs[1], PathSegment::LineTo(30.0, 40.0));
    }

    #[test]
    fn test_decimal_point_separation() {
        // "1.5.7" should parse as 1.5 and 0.7
        let segs = parse_path_data("M1.5.7 0 0").unwrap();
        assert_eq!(segs[0], PathSegment::MoveTo(1.5, 0.7));
    }

    #[test]
    fn test_multiple_h_values() {
        let segs = parse_path_data("M 0 0 H 10 20 30").unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[1], PathSegment::LineTo(10.0, 0.0));
        assert_eq!(segs[2], PathSegment::LineTo(20.0, 0.0));
        assert_eq!(segs[3], PathSegment::LineTo(30.0, 0.0));
    }

    #[test]
    fn test_multiple_v_values() {
        let segs = parse_path_data("M 0 0 V 10 20 30").unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[1], PathSegment::LineTo(0.0, 10.0));
        assert_eq!(segs[2], PathSegment::LineTo(0.0, 20.0));
        assert_eq!(segs[3], PathSegment::LineTo(0.0, 30.0));
    }

    #[test]
    fn test_arc_large_arc_flag() {
        // large_arc=1, sweep=0
        let segs = parse_path_data("M 0 0 A 50 50 0 1 0 100 0").unwrap();
        assert!(segs.len() >= 2);
        match segs.last().unwrap() {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(*end_x, 100.0, 0.1);
                assert_close_f32(*end_y, 0.0, 0.1);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_empty_path() {
        let segs = parse_path_data("").unwrap();
        assert!(segs.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let segs = parse_path_data("   \n\t  ").unwrap();
        assert!(segs.is_empty());
    }

    #[test]
    fn test_scientific_notation() {
        let segs = parse_path_data("M 1e2 2.5E1").unwrap();
        assert_eq!(segs[0], PathSegment::MoveTo(100.0, 25.0));
    }

    #[test]
    fn test_repeated_cubic() {
        let segs =
            parse_path_data("M 0 0 C 1 2 3 4 5 6 7 8 9 10 11 12").unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(
            segs[2],
            PathSegment::CubicTo {
                ctrl1_x: 7.0,
                ctrl1_y: 8.0,
                ctrl2_x: 9.0,
                ctrl2_y: 10.0,
                end_x: 11.0,
                end_y: 12.0,
            }
        );
    }

    #[test]
    fn test_relative_arc() {
        let segs = parse_path_data("M 50 50 a 25 25 0 0 1 50 0").unwrap();
        assert!(segs.len() >= 2);
        match segs.last().unwrap() {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(*end_x, 100.0, 0.1); // 50+50
                assert_close_f32(*end_y, 50.0, 0.1);  // 50+0
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_arc_with_rotation() {
        let segs = parse_path_data("M 0 0 A 100 50 30 0 1 100 0").unwrap();
        assert!(segs.len() >= 2);
        match segs.last().unwrap() {
            PathSegment::CubicTo { end_x, end_y, .. } => {
                assert_close_f32(*end_x, 100.0, 0.1);
                assert_close_f32(*end_y, 0.0, 0.1);
            }
            _ => panic!("Expected CubicTo"),
        }
    }

    #[test]
    fn test_arc_flags_no_separator() {
        // arc flags can appear without separators: "A25 25 0 01 50 0"
        let segs = parse_path_data("M 0 0 A25 25 0 01 50 0").unwrap();
        assert!(segs.len() >= 2);
    }
}

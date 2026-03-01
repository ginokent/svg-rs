use crate::types::Affine2D;

/// SVG transform 属性文字列をパースし、合成済みの Affine2D 行列を返す。
///
/// 対応する変換関数:
///   translate(tx[, ty])  — ty 省略時は 0
///   scale(sx[, sy])      — sy 省略時は sx と同値
///   rotate(deg[, cx, cy])
///   skewX(deg)
///   skewY(deg)
///   matrix(a, b, c, d, e, f)
///
/// 複数の変換が並ぶ場合は左から順に乗算 (行列の右結合):
///   "translate(10,20) rotate(45)" → translate * rotate
///
/// 空文字列やパース不能な入力に対しては Affine2D::identity() を返す。
pub fn parse_transform(attr: &str) -> Affine2D {
    let attr = attr.trim();
    if attr.is_empty() {
        return Affine2D::identity();
    }

    let mut result = Affine2D::identity();
    let mut pos = 0;
    let bytes = attr.as_bytes();

    while pos < bytes.len() {
        // 先頭の空白をスキップ
        pos = skip_whitespace_and_commas(bytes, pos);
        if pos >= bytes.len() {
            break;
        }

        // 関数名を読み取る (英字のみ)
        let name_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_alphabetic() {
            pos += 1;
        }
        if pos == name_start {
            // 関数名が見つからない — パース不能なので identity を返す
            return Affine2D::identity();
        }
        let name = &attr[name_start..pos];

        // '(' までの空白をスキップ
        pos = skip_whitespace(bytes, pos);
        if pos >= bytes.len() || bytes[pos] != b'(' {
            return Affine2D::identity();
        }
        pos += 1; // '(' を消費

        // ')' までの引数文字列を取り出す
        let args_start = pos;
        while pos < bytes.len() && bytes[pos] != b')' {
            pos += 1;
        }
        if pos >= bytes.len() {
            return Affine2D::identity();
        }
        let args_str = &attr[args_start..pos];
        pos += 1; // ')' を消費

        // 引数をパース (カンマまたは空白で区切られた数値列)
        let args = parse_number_list(args_str);

        // 変換関数ごとに Affine2D を生成し、結果に右から乗算
        let mat = match name {
            "translate" => match args.len() {
                1 => Affine2D::translate(args[0], 0.0),
                2 => Affine2D::translate(args[0], args[1]),
                _ => return Affine2D::identity(),
            },
            "scale" => match args.len() {
                1 => Affine2D::scale(args[0], args[0]),
                2 => Affine2D::scale(args[0], args[1]),
                _ => return Affine2D::identity(),
            },
            "rotate" => match args.len() {
                1 => Affine2D::rotate(args[0]),
                3 => Affine2D::rotate_around(args[0], args[1], args[2]),
                _ => return Affine2D::identity(),
            },
            "skewX" => {
                if args.len() != 1 {
                    return Affine2D::identity();
                }
                Affine2D::skew_x(args[0])
            }
            "skewY" => {
                if args.len() != 1 {
                    return Affine2D::identity();
                }
                Affine2D::skew_y(args[0])
            }
            "matrix" => {
                if args.len() != 6 {
                    return Affine2D::identity();
                }
                Affine2D {
                    a: args[0],
                    b: args[1],
                    c: args[2],
                    d: args[3],
                    e: args[4],
                    f: args[5],
                }
            }
            _ => return Affine2D::identity(),
        };

        result = result.multiply(&mat);
    }

    result
}

/// 数値リストをパースする。区切り文字はカンマまたは空白。
/// 負号 '-' は新しい数値の開始として扱う (例: "10-20" → [10, -20])。
fn parse_number_list(s: &str) -> Vec<f32> {
    let mut nums = Vec::new();
    let bytes = s.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        pos = skip_whitespace_and_commas(bytes, pos);
        if pos >= bytes.len() {
            break;
        }

        // 数値の開始位置
        let start = pos;

        // 先頭の符号
        if pos < bytes.len() && (bytes[pos] == b'-' || bytes[pos] == b'+') {
            pos += 1;
        }

        // 整数部
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }

        // 小数部
        if pos < bytes.len() && bytes[pos] == b'.' {
            pos += 1;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
        }

        // 指数部 (例: 1e-3)
        if pos < bytes.len() && (bytes[pos] == b'e' || bytes[pos] == b'E') {
            pos += 1;
            if pos < bytes.len() && (bytes[pos] == b'-' || bytes[pos] == b'+') {
                pos += 1;
            }
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
        }

        if pos == start {
            // 数値を読めなかった — 残りを無視
            break;
        }

        if let Ok(n) = s[start..pos].parse::<f32>() {
            nums.push(n);
        } else {
            break;
        }
    }

    nums
}

/// 空白文字をスキップして次の非空白位置を返す。
fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r') {
        pos += 1;
    }
    pos
}

/// 空白文字およびカンマをスキップして次の位置を返す。
fn skip_whitespace_and_commas(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len()
        && (bytes[pos] == b' '
            || bytes[pos] == b'\t'
            || bytes[pos] == b'\n'
            || bytes[pos] == b'\r'
            || bytes[pos] == b',')
    {
        pos += 1;
    }
    pos
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    /// f32 の比較用ヘルパー。絶対誤差 epsilon 以内なら等しいとみなす。
    fn assert_affine_eq(actual: &Affine2D, expected: &Affine2D, epsilon: f32) {
        assert!(
            (actual.a - expected.a).abs() < epsilon
                && (actual.b - expected.b).abs() < epsilon
                && (actual.c - expected.c).abs() < epsilon
                && (actual.d - expected.d).abs() < epsilon
                && (actual.e - expected.e).abs() < epsilon
                && (actual.f - expected.f).abs() < epsilon,
            "Affine2D mismatch:\n  actual:   {:?}\n  expected: {:?}",
            actual,
            expected,
        );
    }

    const EPS: f32 = 1e-5;

    // ---- translate ----

    #[test]
    fn translate_two_args() {
        let m = parse_transform("translate(10, 20)");
        assert_affine_eq(&m, &Affine2D::translate(10.0, 20.0), EPS);
    }

    #[test]
    fn translate_one_arg() {
        let m = parse_transform("translate(10)");
        assert_affine_eq(&m, &Affine2D::translate(10.0, 0.0), EPS);
    }

    // ---- scale ----

    #[test]
    fn scale_one_arg() {
        let m = parse_transform("scale(2)");
        assert_affine_eq(&m, &Affine2D::scale(2.0, 2.0), EPS);
    }

    #[test]
    fn scale_two_args() {
        let m = parse_transform("scale(2, 3)");
        assert_affine_eq(&m, &Affine2D::scale(2.0, 3.0), EPS);
    }

    // ---- rotate ----

    #[test]
    fn rotate_one_arg() {
        let m = parse_transform("rotate(45)");
        assert_affine_eq(&m, &Affine2D::rotate(45.0), EPS);
    }

    #[test]
    fn rotate_three_args() {
        let m = parse_transform("rotate(45, 100, 200)");
        assert_affine_eq(&m, &Affine2D::rotate_around(45.0, 100.0, 200.0), EPS);
    }

    // ---- skewX / skewY ----

    #[test]
    fn skew_x() {
        let m = parse_transform("skewX(30)");
        assert_affine_eq(&m, &Affine2D::skew_x(30.0), EPS);
    }

    #[test]
    fn skew_y() {
        let m = parse_transform("skewY(30)");
        assert_affine_eq(&m, &Affine2D::skew_y(30.0), EPS);
    }

    // ---- matrix ----

    #[test]
    fn matrix_direct() {
        let m = parse_transform("matrix(1, 0, 0, 1, 50, 100)");
        let expected = Affine2D {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 50.0,
            f: 100.0,
        };
        assert_affine_eq(&m, &expected, EPS);
    }

    // ---- 複数変換の連結 ----

    #[test]
    fn multiple_transforms() {
        let m = parse_transform("translate(10, 20) rotate(45)");
        let expected = Affine2D::translate(10.0, 20.0).multiply(&Affine2D::rotate(45.0));
        assert_affine_eq(&m, &expected, EPS);
    }

    // ---- checkout.svg 由来のケース ----

    #[test]
    fn translate_from_checkout_svg() {
        let m = parse_transform("translate(200, 200)");
        assert_affine_eq(&m, &Affine2D::translate(200.0, 200.0), EPS);
    }

    #[test]
    fn translate_negative_from_checkout_svg_1() {
        let m = parse_transform("translate(-2, -6)");
        assert_affine_eq(&m, &Affine2D::translate(-2.0, -6.0), EPS);
    }

    #[test]
    fn translate_negative_from_checkout_svg_2() {
        let m = parse_transform("translate(-8, 16)");
        assert_affine_eq(&m, &Affine2D::translate(-8.0, 16.0), EPS);
    }

    // ---- 空文字列・不正入力 ----

    #[test]
    fn empty_string_returns_identity() {
        let m = parse_transform("");
        assert_eq!(m, Affine2D::identity());
    }

    #[test]
    fn whitespace_only_returns_identity() {
        let m = parse_transform("   ");
        assert_eq!(m, Affine2D::identity());
    }

    #[test]
    fn invalid_string_returns_identity() {
        let m = parse_transform("not_a_transform");
        assert_eq!(m, Affine2D::identity());
    }

    #[test]
    fn malformed_parens_returns_identity() {
        let m = parse_transform("translate(10, 20");
        assert_eq!(m, Affine2D::identity());
    }
}

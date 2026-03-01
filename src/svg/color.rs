use crate::types::Color;

/// CSS Named Colors の全 148 色テーブル。
/// 名前は小文字で格納し、バイナリサーチ用にアルファベット順にソート済み。
const NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("aliceblue", [240, 248, 255]),
    ("antiquewhite", [250, 235, 215]),
    ("aqua", [0, 255, 255]),
    ("aquamarine", [127, 255, 212]),
    ("azure", [240, 255, 255]),
    ("beige", [245, 245, 220]),
    ("bisque", [255, 228, 196]),
    ("black", [0, 0, 0]),
    ("blanchedalmond", [255, 235, 205]),
    ("blue", [0, 0, 255]),
    ("blueviolet", [138, 43, 226]),
    ("brown", [165, 42, 42]),
    ("burlywood", [222, 184, 135]),
    ("cadetblue", [95, 158, 160]),
    ("chartreuse", [127, 255, 0]),
    ("chocolate", [210, 105, 30]),
    ("coral", [255, 127, 80]),
    ("cornflowerblue", [100, 149, 237]),
    ("cornsilk", [255, 248, 220]),
    ("crimson", [220, 20, 60]),
    ("cyan", [0, 255, 255]),
    ("darkblue", [0, 0, 139]),
    ("darkcyan", [0, 139, 139]),
    ("darkgoldenrod", [184, 134, 11]),
    ("darkgray", [169, 169, 169]),
    ("darkgreen", [0, 100, 0]),
    ("darkgrey", [169, 169, 169]),
    ("darkkhaki", [189, 183, 107]),
    ("darkmagenta", [139, 0, 139]),
    ("darkolivegreen", [85, 107, 47]),
    ("darkorange", [255, 140, 0]),
    ("darkorchid", [153, 50, 204]),
    ("darkred", [139, 0, 0]),
    ("darksalmon", [233, 150, 122]),
    ("darkseagreen", [143, 188, 143]),
    ("darkslateblue", [72, 61, 139]),
    ("darkslategray", [47, 79, 79]),
    ("darkslategrey", [47, 79, 79]),
    ("darkturquoise", [0, 206, 209]),
    ("darkviolet", [148, 0, 211]),
    ("deeppink", [255, 20, 147]),
    ("deepskyblue", [0, 191, 255]),
    ("dimgray", [105, 105, 105]),
    ("dimgrey", [105, 105, 105]),
    ("dodgerblue", [30, 144, 255]),
    ("firebrick", [178, 34, 34]),
    ("floralwhite", [255, 250, 240]),
    ("forestgreen", [34, 139, 34]),
    ("fuchsia", [255, 0, 255]),
    ("gainsboro", [220, 220, 220]),
    ("ghostwhite", [248, 248, 255]),
    ("gold", [255, 215, 0]),
    ("goldenrod", [218, 165, 32]),
    ("gray", [128, 128, 128]),
    ("green", [0, 128, 0]),
    ("greenyellow", [173, 255, 47]),
    ("grey", [128, 128, 128]),
    ("honeydew", [240, 255, 240]),
    ("hotpink", [255, 105, 180]),
    ("indianred", [205, 92, 92]),
    ("indigo", [75, 0, 130]),
    ("ivory", [255, 255, 240]),
    ("khaki", [240, 230, 140]),
    ("lavender", [230, 230, 250]),
    ("lavenderblush", [255, 240, 245]),
    ("lawngreen", [124, 252, 0]),
    ("lemonchiffon", [255, 250, 205]),
    ("lightblue", [173, 216, 230]),
    ("lightcoral", [240, 128, 128]),
    ("lightcyan", [224, 255, 255]),
    ("lightgoldenrodyellow", [250, 250, 210]),
    ("lightgray", [211, 211, 211]),
    ("lightgreen", [144, 238, 144]),
    ("lightgrey", [211, 211, 211]),
    ("lightpink", [255, 182, 193]),
    ("lightsalmon", [255, 160, 122]),
    ("lightseagreen", [32, 178, 170]),
    ("lightskyblue", [135, 206, 250]),
    ("lightslategray", [119, 136, 153]),
    ("lightslategrey", [119, 136, 153]),
    ("lightsteelblue", [176, 196, 222]),
    ("lightyellow", [255, 255, 224]),
    ("lime", [0, 255, 0]),
    ("limegreen", [50, 205, 50]),
    ("linen", [250, 240, 230]),
    ("magenta", [255, 0, 255]),
    ("maroon", [128, 0, 0]),
    ("mediumaquamarine", [102, 205, 170]),
    ("mediumblue", [0, 0, 205]),
    ("mediumorchid", [186, 85, 211]),
    ("mediumpurple", [147, 112, 219]),
    ("mediumseagreen", [60, 179, 113]),
    ("mediumslateblue", [123, 104, 238]),
    ("mediumspringgreen", [0, 250, 154]),
    ("mediumturquoise", [72, 209, 204]),
    ("mediumvioletred", [199, 21, 133]),
    ("midnightblue", [25, 25, 112]),
    ("mintcream", [245, 255, 250]),
    ("mistyrose", [255, 228, 225]),
    ("moccasin", [255, 228, 181]),
    ("navajowhite", [255, 222, 173]),
    ("navy", [0, 0, 128]),
    ("oldlace", [253, 245, 230]),
    ("olive", [128, 128, 0]),
    ("olivedrab", [107, 142, 35]),
    ("orange", [255, 165, 0]),
    ("orangered", [255, 69, 0]),
    ("orchid", [218, 112, 214]),
    ("palegoldenrod", [238, 232, 170]),
    ("palegreen", [152, 251, 152]),
    ("paleturquoise", [175, 238, 238]),
    ("palevioletred", [219, 112, 147]),
    ("papayawhip", [255, 239, 213]),
    ("peachpuff", [255, 218, 185]),
    ("peru", [205, 133, 63]),
    ("pink", [255, 192, 203]),
    ("plum", [221, 160, 221]),
    ("powderblue", [176, 224, 230]),
    ("purple", [128, 0, 128]),
    ("rebeccapurple", [102, 51, 153]),
    ("red", [255, 0, 0]),
    ("rosybrown", [188, 143, 143]),
    ("royalblue", [65, 105, 225]),
    ("saddlebrown", [139, 69, 19]),
    ("salmon", [250, 128, 114]),
    ("sandybrown", [244, 164, 96]),
    ("seagreen", [46, 139, 87]),
    ("seashell", [255, 245, 238]),
    ("sienna", [160, 82, 45]),
    ("silver", [192, 192, 192]),
    ("skyblue", [135, 206, 235]),
    ("slateblue", [106, 90, 205]),
    ("slategray", [112, 128, 144]),
    ("slategrey", [112, 128, 144]),
    ("snow", [255, 250, 250]),
    ("springgreen", [0, 255, 127]),
    ("steelblue", [70, 130, 180]),
    ("tan", [210, 180, 140]),
    ("teal", [0, 128, 128]),
    ("thistle", [216, 191, 216]),
    ("tomato", [255, 99, 71]),
    ("turquoise", [64, 224, 208]),
    ("violet", [238, 130, 238]),
    ("wheat", [245, 222, 179]),
    ("white", [255, 255, 255]),
    ("whitesmoke", [245, 245, 245]),
    ("yellow", [255, 255, 0]),
    ("yellowgreen", [154, 205, 50]),
];

/// 色文字列をパースする。"none" の場合は None を返す。
///
/// 対応フォーマット:
/// - `#RGB` — 各桁を 2 回繰り返して展開, a=255
/// - `#RRGGBB` — そのままパース, a=255
/// - `#RRGGBBAA` — そのままパース
/// - `rgb(r, g, b)` — 整数 0-255, a=255
/// - `rgb(r%, g%, b%)` — パーセンテージを 0-255 に変換, a=255
/// - `rgba(r, g, b, a)` — r,g,b: 0-255, a: float 0.0-1.0 を 0-255 にマッピング
/// - `none` — None を返す
/// - CSS Named Colors — 148 色
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();

    // "none" チェック (大文字小文字を区別しない)
    if s.eq_ignore_ascii_case("none") {
        return None;
    }

    // hex 形式: #RGB, #RRGGBB, #RRGGBBAA
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }

    // rgb(...) / rgba(...) 関数形式
    if let Some(inner) = strip_func(s, "rgba") {
        return parse_rgba(inner);
    }
    if let Some(inner) = strip_func(s, "rgb") {
        return parse_rgb(inner);
    }

    // CSS Named Colors (大文字小文字を区別しないバイナリサーチ)
    lookup_named_color(s)
}

/// hex 文字列 (# を除去済み) をパースする。
fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        // #RGB → 各桁を 2 回繰り返す
        3 => {
            let r = parse_hex_digit(hex.as_bytes()[0])?;
            let g = parse_hex_digit(hex.as_bytes()[1])?;
            let b = parse_hex_digit(hex.as_bytes()[2])?;
            Some(Color {
                r: r * 17, // 0xF * 17 = 0xFF
                g: g * 17,
                b: b * 17,
                a: 255,
            })
        }
        // #RRGGBB
        6 => {
            let r = parse_hex_byte(&hex[0..2])?;
            let g = parse_hex_byte(&hex[2..4])?;
            let b = parse_hex_byte(&hex[4..6])?;
            Some(Color { r, g, b, a: 255 })
        }
        // #RRGGBBAA
        8 => {
            let r = parse_hex_byte(&hex[0..2])?;
            let g = parse_hex_byte(&hex[2..4])?;
            let b = parse_hex_byte(&hex[4..6])?;
            let a = parse_hex_byte(&hex[6..8])?;
            Some(Color { r, g, b, a })
        }
        _ => None,
    }
}

/// 1 文字の hex digit を 0-15 の u8 に変換する。
fn parse_hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// 2 文字の hex 文字列を u8 に変換する。
fn parse_hex_byte(s: &str) -> Option<u8> {
    let hi = parse_hex_digit(s.as_bytes()[0])?;
    let lo = parse_hex_digit(s.as_bytes()[1])?;
    Some(hi * 16 + lo)
}

/// 関数形式 `name(...)` から括弧内の文字列を取り出す。
/// 大文字小文字を区別しない。
fn strip_func<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let s_lower = s.to_ascii_lowercase();
    if !s_lower.starts_with(name) {
        return None;
    }
    let rest = &s[name.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('(')?;
    let rest = rest.strip_suffix(')')?;
    Some(rest)
}

/// `rgb(r, g, b)` をパースする。
/// 整数値またはパーセンテージ値に対応。
fn parse_rgb(inner: &str) -> Option<Color> {
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return None;
    }

    let r = parse_color_component(parts[0].trim())?;
    let g = parse_color_component(parts[1].trim())?;
    let b = parse_color_component(parts[2].trim())?;

    Some(Color { r, g, b, a: 255 })
}

/// `rgba(r, g, b, a)` をパースする。
/// r,g,b: 整数 0-255, a: float 0.0-1.0。
fn parse_rgba(inner: &str) -> Option<Color> {
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 4 {
        return None;
    }

    let r = parse_color_component(parts[0].trim())?;
    let g = parse_color_component(parts[1].trim())?;
    let b = parse_color_component(parts[2].trim())?;

    // alpha は 0.0-1.0 の float を 0-255 にマッピング
    let a_float: f32 = parts[3].trim().parse().ok()?;
    let a = (a_float * 255.0).round() as u8;

    Some(Color { r, g, b, a })
}

/// 色コンポーネント文字列をパースする。
/// - `"50%"` のようなパーセンテージ → 0-255 にマッピング
/// - `"128"` のような整数 → そのまま
fn parse_color_component(s: &str) -> Option<u8> {
    if let Some(pct_str) = s.strip_suffix('%') {
        // パーセンテージ: 0%-100% → 0-255
        let pct: f32 = pct_str.trim().parse().ok()?;
        Some((pct / 100.0 * 255.0).round() as u8)
    } else {
        // 整数値
        let v: u8 = s.parse().ok()?;
        Some(v)
    }
}

/// Named Colors テーブルからバイナリサーチで色を検索する。
/// 大文字小文字を区別しない。
fn lookup_named_color(name: &str) -> Option<Color> {
    // 入力を小文字に変換してバイナリサーチ
    let lower = name.to_ascii_lowercase();
    match NAMED_COLORS.binary_search_by_key(&&*lower, |(n, _)| n) {
        Ok(idx) => {
            let [r, g, b] = NAMED_COLORS[idx].1;
            Some(Color { r, g, b, a: 255 })
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // hex 形式テスト
    // ========================================

    #[test]
    fn test_hex_rgb_short() {
        // #RGB → 各桁を 2 回繰り返す
        assert_eq!(
            parse_color("#F00"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("#0F0"),
            Some(Color { r: 0, g: 255, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("#00F"),
            Some(Color { r: 0, g: 0, b: 255, a: 255 })
        );
        assert_eq!(
            parse_color("#ABC"),
            Some(Color { r: 0xAA, g: 0xBB, b: 0xCC, a: 255 })
        );
    }

    #[test]
    fn test_hex_rrggbb() {
        assert_eq!(
            parse_color("#FF0000"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("#00FF00"),
            Some(Color { r: 0, g: 255, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("#0000FF"),
            Some(Color { r: 0, g: 0, b: 255, a: 255 })
        );
    }

    #[test]
    fn test_hex_rrggbbaa() {
        assert_eq!(
            parse_color("#FF000080"),
            Some(Color { r: 255, g: 0, b: 0, a: 128 })
        );
        assert_eq!(
            parse_color("#00FF00FF"),
            Some(Color { r: 0, g: 255, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("#0000FF00"),
            Some(Color { r: 0, g: 0, b: 255, a: 0 })
        );
    }

    // ========================================
    // rgb() テスト
    // ========================================

    #[test]
    fn test_rgb_integers() {
        assert_eq!(
            parse_color("rgb(255, 0, 0)"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("rgb(0,128,255)"),
            Some(Color { r: 0, g: 128, b: 255, a: 255 })
        );
    }

    #[test]
    fn test_rgb_percentages() {
        assert_eq!(
            parse_color("rgb(100%, 0%, 0%)"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("rgb(0%, 50%, 100%)"),
            Some(Color { r: 0, g: 128, b: 255, a: 255 })
        );
    }

    // ========================================
    // rgba() テスト
    // ========================================

    #[test]
    fn test_rgba() {
        assert_eq!(
            parse_color("rgba(255, 0, 0, 1.0)"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("rgba(255, 0, 0, 0.5)"),
            Some(Color { r: 255, g: 0, b: 0, a: 128 })
        );
        assert_eq!(
            parse_color("rgba(0, 0, 0, 0.0)"),
            Some(Color { r: 0, g: 0, b: 0, a: 0 })
        );
    }

    // ========================================
    // "none" テスト
    // ========================================

    #[test]
    fn test_none() {
        assert_eq!(parse_color("none"), None);
        assert_eq!(parse_color("None"), None);
        assert_eq!(parse_color("NONE"), None);
    }

    // ========================================
    // Named Colors テスト
    // ========================================

    #[test]
    fn test_named_colors() {
        assert_eq!(
            parse_color("red"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("blue"),
            Some(Color { r: 0, g: 0, b: 255, a: 255 })
        );
        assert_eq!(
            parse_color("green"),
            Some(Color { r: 0, g: 128, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("white"),
            Some(Color { r: 255, g: 255, b: 255, a: 255 })
        );
        assert_eq!(
            parse_color("black"),
            Some(Color { r: 0, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("aliceblue"),
            Some(Color { r: 240, g: 248, b: 255, a: 255 })
        );
        assert_eq!(
            parse_color("yellowgreen"),
            Some(Color { r: 154, g: 205, b: 50, a: 255 })
        );
    }

    #[test]
    fn test_named_colors_case_insensitive() {
        assert_eq!(
            parse_color("Red"),
            Some(Color { r: 255, g: 0, b: 0, a: 255 })
        );
        assert_eq!(
            parse_color("BLUE"),
            Some(Color { r: 0, g: 0, b: 255, a: 255 })
        );
        assert_eq!(
            parse_color("AliceBlue"),
            Some(Color { r: 240, g: 248, b: 255, a: 255 })
        );
    }

    // ========================================
    // checkout.svg で使用される色のテスト
    // ========================================

    #[test]
    fn test_checkout_svg_colors() {
        assert_eq!(
            parse_color("#F8FAFC"),
            Some(Color { r: 0xF8, g: 0xFA, b: 0xFC, a: 255 })
        );
        assert_eq!(
            parse_color("#F59E0B"),
            Some(Color { r: 0xF5, g: 0x9E, b: 0x0B, a: 255 })
        );
        assert_eq!(
            parse_color("#D97706"),
            Some(Color { r: 0xD9, g: 0x77, b: 0x06, a: 255 })
        );
        assert_eq!(
            parse_color("#FEF3C7"),
            Some(Color { r: 0xFE, g: 0xF3, b: 0xC7, a: 255 })
        );
        assert_eq!(
            parse_color("#6366F1"),
            Some(Color { r: 0x63, g: 0x66, b: 0xF1, a: 255 })
        );
        assert_eq!(
            parse_color("#312E81"),
            Some(Color { r: 0x31, g: 0x2E, b: 0x81, a: 255 })
        );
        assert_eq!(
            parse_color("#FBBF24"),
            Some(Color { r: 0xFB, g: 0xBF, b: 0x24, a: 255 })
        );
        assert_eq!(
            parse_color("#F43F5E"),
            Some(Color { r: 0xF4, g: 0x3F, b: 0x5E, a: 255 })
        );
        assert_eq!(
            parse_color("#10B981"),
            Some(Color { r: 0x10, g: 0xB9, b: 0x81, a: 255 })
        );
        assert_eq!(
            parse_color("#FFFFFF"),
            Some(Color { r: 0xFF, g: 0xFF, b: 0xFF, a: 255 })
        );
        assert_eq!(
            parse_color("#475569"),
            Some(Color { r: 0x47, g: 0x55, b: 0x69, a: 255 })
        );
    }

    #[test]
    fn test_checkout_svg_rgba() {
        // rgba(0,0,0,0.08) → a = round(0.08 * 255) = round(20.4) = 20
        assert_eq!(
            parse_color("rgba(0,0,0,0.08)"),
            Some(Color { r: 0, g: 0, b: 0, a: 20 })
        );
    }
}

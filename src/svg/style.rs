use crate::types::*;
use crate::xml::XmlElement;

/// fill 属性群をパースして FillStyle を返す。
/// fill="none" の場合は None を返す。
/// fill 属性が省略された場合は SVG デフォルトの黒 (#000000) を使う。
pub fn parse_fill(el: &XmlElement) -> Option<FillStyle> {
    let fill_str = el.attribute("fill").unwrap_or("#000000");

    // "none" はフィルなしを意味する
    if fill_str.eq_ignore_ascii_case("none") {
        return None;
    }

    let color = crate::svg::color::parse_color(fill_str)?;

    let rule = match el.attribute("fill-rule") {
        Some(s) if s.eq_ignore_ascii_case("evenodd") => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    };

    let opacity = parse_f32_attr(el, "fill-opacity", 1.0).clamp(0.0, 1.0);

    Some(FillStyle {
        color,
        rule,
        opacity,
    })
}

/// stroke 属性群をパースして StrokeStyle を返す。
/// stroke 属性が省略された場合、または "none" の場合は None を返す。
pub fn parse_stroke(el: &XmlElement) -> Option<StrokeStyle> {
    let stroke_str = el.attribute("stroke")?;

    // "none" はストロークなしを意味する
    if stroke_str.eq_ignore_ascii_case("none") {
        return None;
    }

    let color = crate::svg::color::parse_color(stroke_str)?;

    let width = parse_f32_attr(el, "stroke-width", 1.0);

    let cap = match el.attribute("stroke-linecap") {
        Some(s) if s.eq_ignore_ascii_case("round") => LineCap::Round,
        Some(s) if s.eq_ignore_ascii_case("square") => LineCap::Square,
        _ => LineCap::Butt,
    };

    let join = match el.attribute("stroke-linejoin") {
        Some(s) if s.eq_ignore_ascii_case("round") => LineJoin::Round,
        Some(s) if s.eq_ignore_ascii_case("bevel") => LineJoin::Bevel,
        _ => LineJoin::Miter,
    };

    let dash = parse_dash_pattern(el);

    let opacity = parse_f32_attr(el, "stroke-opacity", 1.0).clamp(0.0, 1.0);

    Some(StrokeStyle {
        color,
        width,
        cap,
        join,
        dash,
        opacity,
    })
}

/// opacity 属性をパースする。省略時は 1.0。
pub fn parse_opacity(el: &XmlElement) -> f32 {
    parse_f32_attr(el, "opacity", 1.0).clamp(0.0, 1.0)
}

// ============================================
// 内部ヘルパー
// ============================================

/// 要素から指定属性を f32 としてパースする。パース失敗時はデフォルト値を返す。
fn parse_f32_attr(el: &XmlElement, name: &str, default: f32) -> f32 {
    el.attribute(name)
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(default)
}

/// stroke-dasharray / stroke-dashoffset をパースして DashPattern を返す。
/// dasharray が "none" または未指定の場合は None を返す。
fn parse_dash_pattern(el: &XmlElement) -> Option<DashPattern> {
    let dasharray_str = el.attribute("stroke-dasharray")?;

    if dasharray_str.eq_ignore_ascii_case("none") {
        return None;
    }

    // dasharray はカンマまたは空白で区切られた数値のリスト
    let array: Vec<f32> = dasharray_str
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();

    if array.is_empty() {
        return None;
    }

    let offset = parse_f32_attr(el, "stroke-dashoffset", 0.0);

    Some(DashPattern { array, offset })
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用ヘルパー: 属性リストから XmlElement を作成する。
    fn make_element(attrs: &[(&str, &str)]) -> XmlElement {
        XmlElement {
            tag: "path".to_string(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children: Vec::new(),
        }
    }

    // ----------------------------------------
    // parse_fill テスト
    // ----------------------------------------

    #[test]
    fn test_default_fill_is_black() {
        // fill 属性なし → デフォルトの黒フィル
        let el = make_element(&[]);
        let fill = parse_fill(&el).expect("should return Some for default fill");
        assert_eq!(fill.color, Color { r: 0, g: 0, b: 0, a: 255 });
        assert_eq!(fill.rule, FillRule::NonZero);
        assert_eq!(fill.opacity, 1.0);
    }

    #[test]
    fn test_fill_none_returns_none() {
        let el = make_element(&[("fill", "none")]);
        assert!(parse_fill(&el).is_none());
    }

    #[test]
    fn test_fill_red_with_opacity() {
        let el = make_element(&[("fill", "#FF0000"), ("fill-opacity", "0.5")]);
        let fill = parse_fill(&el).expect("should return Some");
        assert_eq!(fill.color, Color { r: 255, g: 0, b: 0, a: 255 });
        assert_eq!(fill.opacity, 0.5);
    }

    #[test]
    fn test_fill_rule_evenodd() {
        let el = make_element(&[("fill", "#000000"), ("fill-rule", "evenodd")]);
        let fill = parse_fill(&el).expect("should return Some");
        assert_eq!(fill.rule, FillRule::EvenOdd);
    }

    #[test]
    fn test_fill_rule_nonzero_default() {
        let el = make_element(&[("fill", "#000000")]);
        let fill = parse_fill(&el).expect("should return Some");
        assert_eq!(fill.rule, FillRule::NonZero);
    }

    // ----------------------------------------
    // parse_stroke テスト
    // ----------------------------------------

    #[test]
    fn test_default_stroke_is_none() {
        // stroke 属性なし → None
        let el = make_element(&[]);
        assert!(parse_stroke(&el).is_none());
    }

    #[test]
    fn test_stroke_none_returns_none() {
        let el = make_element(&[("stroke", "none")]);
        assert!(parse_stroke(&el).is_none());
    }

    #[test]
    fn test_stroke_with_attributes() {
        let el = make_element(&[
            ("stroke", "#475569"),
            ("stroke-width", "2.5"),
            ("stroke-linecap", "round"),
            ("stroke-linejoin", "bevel"),
        ]);
        let stroke = parse_stroke(&el).expect("should return Some");
        assert_eq!(stroke.color, Color { r: 0x47, g: 0x55, b: 0x69, a: 255 });
        assert_eq!(stroke.width, 2.5);
        assert_eq!(stroke.cap, LineCap::Round);
        assert_eq!(stroke.join, LineJoin::Bevel);
        assert_eq!(stroke.opacity, 1.0);
    }

    #[test]
    fn test_stroke_linecap_square() {
        let el = make_element(&[("stroke", "#000"), ("stroke-linecap", "square")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        assert_eq!(stroke.cap, LineCap::Square);
    }

    #[test]
    fn test_stroke_linejoin_round() {
        let el = make_element(&[("stroke", "#000"), ("stroke-linejoin", "round")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        assert_eq!(stroke.join, LineJoin::Round);
    }

    #[test]
    fn test_stroke_dasharray_single_value() {
        // checkout.svg スタイルの dasharray="50"
        let el = make_element(&[("stroke", "#000"), ("stroke-dasharray", "50")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        let dash = stroke.dash.expect("dash should be Some");
        assert_eq!(dash.array, vec![50.0]);
        assert_eq!(dash.offset, 0.0);
    }

    #[test]
    fn test_stroke_dasharray_multiple_values() {
        let el = make_element(&[("stroke", "#000"), ("stroke-dasharray", "5,10,15")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        let dash = stroke.dash.expect("dash should be Some");
        assert_eq!(dash.array, vec![5.0, 10.0, 15.0]);
    }

    #[test]
    fn test_stroke_dasharray_space_separated() {
        let el = make_element(&[("stroke", "#000"), ("stroke-dasharray", "5 10 15")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        let dash = stroke.dash.expect("dash should be Some");
        assert_eq!(dash.array, vec![5.0, 10.0, 15.0]);
    }

    #[test]
    fn test_stroke_dasharray_none() {
        let el = make_element(&[("stroke", "#000"), ("stroke-dasharray", "none")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        assert!(stroke.dash.is_none());
    }

    #[test]
    fn test_stroke_dashoffset() {
        let el = make_element(&[
            ("stroke", "#000"),
            ("stroke-dasharray", "50"),
            ("stroke-dashoffset", "25.5"),
        ]);
        let stroke = parse_stroke(&el).expect("should return Some");
        let dash = stroke.dash.expect("dash should be Some");
        assert_eq!(dash.offset, 25.5);
    }

    #[test]
    fn test_stroke_opacity() {
        let el = make_element(&[("stroke", "#000"), ("stroke-opacity", "0.3")]);
        let stroke = parse_stroke(&el).expect("should return Some");
        assert!((stroke.opacity - 0.3).abs() < f32::EPSILON);
    }

    // ----------------------------------------
    // parse_opacity テスト
    // ----------------------------------------

    #[test]
    fn test_opacity_default() {
        let el = make_element(&[]);
        assert_eq!(parse_opacity(&el), 1.0);
    }

    #[test]
    fn test_opacity_attribute() {
        let el = make_element(&[("opacity", "0.75")]);
        assert_eq!(parse_opacity(&el), 0.75);
    }

    #[test]
    fn test_opacity_clamped_above_one() {
        let el = make_element(&[("opacity", "2.0")]);
        assert_eq!(parse_opacity(&el), 1.0);
    }

    #[test]
    fn test_opacity_clamped_below_zero() {
        let el = make_element(&[("opacity", "-0.5")]);
        assert_eq!(parse_opacity(&el), 0.0);
    }
}

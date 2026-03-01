use std::collections::HashMap;

use crate::types::*;
use crate::xml::XmlElement;

// ============================================
// StyleContext: 親→子のスタイル継承
// ============================================

/// 親から子へ継承されるスタイル属性群。
/// CSS specificity に従い、inline style > presentation attributes > inherited values > defaults の順で解決する。
#[derive(Debug, Clone)]
pub struct StyleContext {
    pub fill: Option<FillStyle>,       // 継承可能 (SVG デフォルト: 黒)
    pub stroke: Option<StrokeStyle>,   // 継承可能 (SVG デフォルト: none)
    pub fill_opacity: f32,
    pub stroke_opacity: f32,
    pub opacity: f32,
    pub visibility: bool,
}

impl StyleContext {
    /// SVG ルートのデフォルト StyleContext。
    pub fn default_root() -> Self {
        Self {
            fill: Some(FillStyle {
                paint: Paint::Color(Color { r: 0, g: 0, b: 0, a: 255 }),
                rule: FillRule::NonZero,
                opacity: 1.0,
            }),
            stroke: None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: true,
        }
    }
}

// ============================================
// inline style パーサー
// ============================================

/// `style` 属性をパースし、プロパティ名→値の HashMap を返す。
/// 例: "fill:#f00; stroke:none; opacity:0.5" → {"fill": "#f00", "stroke": "none", "opacity": "0.5"}
pub fn parse_inline_style(style_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for decl in style_str.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some((prop, val)) = decl.split_once(':') {
            map.insert(prop.trim().to_lowercase(), val.trim().to_string());
        }
    }
    map
}

// ============================================
// url() 参照の ID 抽出
// ============================================

/// fill/stroke 値から url() 参照の ID を抽出する。
/// 例: "url(#grad1)" → Some("grad1"), "red" → None
pub fn extract_url_id(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with("url(") && s.ends_with(')') {
        let inner = s[4..s.len() - 1].trim();
        let inner = inner.trim_matches(|c| c == '"' || c == '\'');
        Some(inner.trim_start_matches('#'))
    } else {
        None
    }
}

// ============================================
// resolve_* 関数群: StyleContext ベースのスタイル解決
// ============================================

/// 要素のフィルスタイルを StyleContext に基づいて解決する。
/// 優先度: inline style > presentation attributes > 継承値 > SVG デフォルト
pub fn resolve_fill(el: &XmlElement, ctx: &StyleContext) -> Option<FillStyle> {
    let inline = el.attribute("style").map(parse_inline_style).unwrap_or_default();

    // fill 値の決定 (inline style > presentation attribute > 継承)
    let fill_str = inline.get("fill")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("fill"));

    match fill_str {
        Some(s) if s.eq_ignore_ascii_case("none") => None,
        Some(s) => {
            // url() 参照の検出 (グラデーション等)
            // Step 6 で解決される。この時点では継承値にフォールバック。
            if s.trim().starts_with("url(") {
                return ctx.fill.clone();
            }

            // 色パースを試みる
            // パース失敗時は継承値にフォールバック
            match crate::svg::color::parse_color(s) {
                Some(color) => {
                    let rule = inline.get("fill-rule")
                        .map(|s| s.as_str())
                        .or_else(|| el.attribute("fill-rule"))
                        .map(|s| if s.eq_ignore_ascii_case("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero })
                        .unwrap_or(FillRule::NonZero);

                    let opacity = inline.get("fill-opacity")
                        .and_then(|s| s.parse::<f32>().ok())
                        .or_else(|| el.attribute("fill-opacity").and_then(|s| s.parse::<f32>().ok()))
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0);

                    Some(FillStyle { paint: Paint::Color(color), rule, opacity })
                }
                None => {
                    // 色パース失敗 → 継承値にフォールバック
                    ctx.fill.clone()
                }
            }
        }
        None => {
            // 要素に fill 指定がない → 継承
            // ただし fill-rule, fill-opacity が要素で指定されている場合は上書き
            let mut inherited = ctx.fill.clone();
            if let Some(ref mut fill) = inherited {
                if let Some(rule_str) = inline.get("fill-rule").map(|s| s.as_str()).or_else(|| el.attribute("fill-rule")) {
                    fill.rule = if rule_str.eq_ignore_ascii_case("evenodd") { FillRule::EvenOdd } else { FillRule::NonZero };
                }
                if let Some(op) = inline.get("fill-opacity").and_then(|s| s.parse::<f32>().ok()).or_else(|| el.attribute("fill-opacity").and_then(|s| s.parse::<f32>().ok())) {
                    fill.opacity = op.clamp(0.0, 1.0);
                }
            }
            inherited
        }
    }
}

/// 要素のストロークスタイルを StyleContext に基づいて解決する。
/// 優先度: inline style > presentation attributes > 継承値
pub fn resolve_stroke(el: &XmlElement, ctx: &StyleContext) -> Option<StrokeStyle> {
    let inline = el.attribute("style").map(parse_inline_style).unwrap_or_default();

    let stroke_str = inline.get("stroke")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("stroke"));

    match stroke_str {
        Some(s) if s.eq_ignore_ascii_case("none") => None,
        Some(s) => {
            // url() 参照の検出 (グラデーション等)
            // Step 6 で解決される。この時点では継承値にフォールバック。
            if s.trim().starts_with("url(") {
                return ctx.stroke.clone();
            }

            match crate::svg::color::parse_color(s) {
                Some(color) => {
                    let width = inline.get("stroke-width")
                        .and_then(|s| s.parse::<f32>().ok())
                        .or_else(|| el.attribute("stroke-width").and_then(|s| s.trim().parse::<f32>().ok()))
                        .unwrap_or(1.0);

                    let cap = inline.get("stroke-linecap")
                        .map(|s| s.as_str())
                        .or_else(|| el.attribute("stroke-linecap"))
                        .map(|s| match s {
                            s if s.eq_ignore_ascii_case("round") => LineCap::Round,
                            s if s.eq_ignore_ascii_case("square") => LineCap::Square,
                            _ => LineCap::Butt,
                        })
                        .unwrap_or(LineCap::Butt);

                    let join = inline.get("stroke-linejoin")
                        .map(|s| s.as_str())
                        .or_else(|| el.attribute("stroke-linejoin"))
                        .map(|s| match s {
                            s if s.eq_ignore_ascii_case("round") => LineJoin::Round,
                            s if s.eq_ignore_ascii_case("bevel") => LineJoin::Bevel,
                            _ => LineJoin::Miter,
                        })
                        .unwrap_or(LineJoin::Miter);

                    let dash = parse_dash_from_inline_or_attr(el, &inline);

                    let opacity = inline.get("stroke-opacity")
                        .and_then(|s| s.parse::<f32>().ok())
                        .or_else(|| el.attribute("stroke-opacity").and_then(|s| s.trim().parse::<f32>().ok()))
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0);

                    Some(StrokeStyle { paint: Paint::Color(color), width, cap, join, dash, opacity })
                }
                None => {
                    // 色パース失敗 → 継承値にフォールバック
                    ctx.stroke.clone()
                }
            }
        }
        None => {
            // 要素に stroke 指定がない → 継承値を使用
            ctx.stroke.clone()
        }
    }
}

/// inline style または presentation attribute から dash pattern を解決する。
fn parse_dash_from_inline_or_attr(el: &XmlElement, inline: &HashMap<String, String>) -> Option<DashPattern> {
    let dasharray_str = inline.get("stroke-dasharray")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("stroke-dasharray"));

    let dasharray_str = dasharray_str?;

    if dasharray_str.eq_ignore_ascii_case("none") {
        return None;
    }

    let array: Vec<f32> = dasharray_str
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();

    if array.is_empty() {
        return None;
    }

    let offset = inline.get("stroke-dashoffset")
        .and_then(|s| s.parse::<f32>().ok())
        .or_else(|| el.attribute("stroke-dashoffset").and_then(|s| s.trim().parse::<f32>().ok()))
        .unwrap_or(0.0);

    Some(DashPattern { array, offset })
}

/// 要素の opacity を StyleContext に基づいて解決する。
/// opacity は CSS 上は非継承プロパティだが、要素のグループ opacity として機能する。
pub fn resolve_opacity(el: &XmlElement, _ctx: &StyleContext) -> f32 {
    let inline = el.attribute("style").map(parse_inline_style).unwrap_or_default();

    inline.get("opacity")
        .and_then(|s| s.parse::<f32>().ok())
        .or_else(|| el.attribute("opacity").and_then(|s| s.trim().parse::<f32>().ok()))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

/// 要素の visibility を StyleContext に基づいて解決する。
/// visibility は継承プロパティ。inline style > presentation attribute > 継承値 の順で解決。
pub fn resolve_visibility(el: &XmlElement, ctx: &StyleContext) -> bool {
    let inline = el.attribute("style").map(parse_inline_style).unwrap_or_default();

    if let Some(vis) = inline.get("visibility") {
        return !vis.eq_ignore_ascii_case("hidden");
    }
    if let Some(vis) = el.attribute("visibility") {
        return !vis.eq_ignore_ascii_case("hidden");
    }
    // 継承
    ctx.visibility
}

/// fill 属性群をパースして FillStyle を返す。
/// fill="none" の場合は None を返す。
/// fill 属性が省略された場合は SVG デフォルトの黒 (#000000) を使う。
/// NOTE: resolve_fill() が StyleContext ベースの新 API。この関数はテストおよび将来の用途のため残す。
#[allow(dead_code)]
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
        paint: Paint::Color(color),
        rule,
        opacity,
    })
}

/// stroke 属性群をパースして StrokeStyle を返す。
/// stroke 属性が省略された場合、または "none" の場合は None を返す。
/// NOTE: resolve_stroke() が StyleContext ベースの新 API。この関数はテストおよび将来の用途のため残す。
#[allow(dead_code)]
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
        paint: Paint::Color(color),
        width,
        cap,
        join,
        dash,
        opacity,
    })
}

/// opacity 属性をパースする。省略時は 1.0。
/// NOTE: resolve_opacity() が StyleContext ベースの新 API。この関数はテストおよび将来の用途のため残す。
#[allow(dead_code)]
pub fn parse_opacity(el: &XmlElement) -> f32 {
    parse_f32_attr(el, "opacity", 1.0).clamp(0.0, 1.0)
}

// ============================================
// 内部ヘルパー
// ============================================

/// 要素から指定属性を f32 としてパースする。パース失敗時はデフォルト値を返す。
#[allow(dead_code)]
fn parse_f32_attr(el: &XmlElement, name: &str, default: f32) -> f32 {
    el.attribute(name)
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(default)
}

/// stroke-dasharray / stroke-dashoffset をパースして DashPattern を返す。
/// dasharray が "none" または未指定の場合は None を返す。
#[allow(dead_code)]
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
        assert_eq!(fill.paint, Paint::Color(Color { r: 0, g: 0, b: 0, a: 255 }));
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
        assert_eq!(fill.paint, Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
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
        assert_eq!(stroke.paint, Paint::Color(Color { r: 0x47, g: 0x55, b: 0x69, a: 255 }));
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

    // ----------------------------------------
    // parse_inline_style テスト
    // ----------------------------------------

    #[test]
    fn test_parse_inline_style_basic() {
        let result = parse_inline_style("fill:#f00; stroke:none; opacity:0.5");
        assert_eq!(result.get("fill"), Some(&"#f00".to_string()));
        assert_eq!(result.get("stroke"), Some(&"none".to_string()));
        assert_eq!(result.get("opacity"), Some(&"0.5".to_string()));
    }

    #[test]
    fn test_parse_inline_style_empty() {
        let result = parse_inline_style("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_inline_style_trailing_semicolon() {
        let result = parse_inline_style("fill:red;");
        assert_eq!(result.get("fill"), Some(&"red".to_string()));
        assert_eq!(result.len(), 1);
    }

    // ----------------------------------------
    // resolve_fill テスト
    // ----------------------------------------

    #[test]
    fn test_resolve_fill_inline_over_attribute() {
        // inline style の fill が presentation attribute より優先される
        let el = make_element(&[("fill", "blue"), ("style", "fill: red")]);
        let ctx = StyleContext::default_root();
        let fill = resolve_fill(&el, &ctx).unwrap();
        assert_eq!(fill.paint, Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
    }

    #[test]
    fn test_resolve_fill_inherited() {
        // 要素に fill がない場合、親から継承
        let el = make_element(&[]);
        let ctx = StyleContext {
            fill: Some(FillStyle {
                paint: Paint::Color(Color { r: 0, g: 128, b: 0, a: 255 }),
                rule: FillRule::NonZero,
                opacity: 1.0,
            }),
            stroke: None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: true,
        };
        let fill = resolve_fill(&el, &ctx).unwrap();
        assert_eq!(fill.paint, Paint::Color(Color { r: 0, g: 128, b: 0, a: 255 }));
    }

    #[test]
    fn test_resolve_fill_none_overrides_inherited() {
        // fill="none" は継承値を上書きして None を返す
        let el = make_element(&[("fill", "none")]);
        let ctx = StyleContext {
            fill: Some(FillStyle {
                paint: Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }),
                rule: FillRule::NonZero,
                opacity: 1.0,
            }),
            stroke: None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: true,
        };
        assert!(resolve_fill(&el, &ctx).is_none());
    }

    // ----------------------------------------
    // resolve_stroke テスト
    // ----------------------------------------

    #[test]
    fn test_resolve_stroke_inherited() {
        // 要素に stroke がない場合、親から継承
        let el = make_element(&[]);
        let ctx = StyleContext {
            fill: None,
            stroke: Some(StrokeStyle {
                paint: Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }),
                width: 2.0,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                dash: None,
                opacity: 1.0,
            }),
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: true,
        };
        let stroke = resolve_stroke(&el, &ctx).unwrap();
        assert_eq!(stroke.paint, Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
        assert_eq!(stroke.width, 2.0);
    }

    #[test]
    fn test_resolve_stroke_none_overrides_inherited() {
        // stroke="none" は継承値を上書き
        let el = make_element(&[("stroke", "none")]);
        let ctx = StyleContext {
            fill: None,
            stroke: Some(StrokeStyle {
                paint: Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }),
                width: 2.0,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
                dash: None,
                opacity: 1.0,
            }),
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: true,
        };
        assert!(resolve_stroke(&el, &ctx).is_none());
    }

    // ----------------------------------------
    // resolve_visibility テスト
    // ----------------------------------------

    #[test]
    fn test_resolve_visibility_inherited() {
        let el = make_element(&[]);
        let ctx = StyleContext {
            fill: None,
            stroke: None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: false, // 親が hidden
        };
        assert!(!resolve_visibility(&el, &ctx));
    }

    #[test]
    fn test_resolve_visibility_inline_overrides_inherited() {
        // inline style で visibility を上書き
        let el = make_element(&[("style", "visibility: visible")]);
        let ctx = StyleContext {
            fill: None,
            stroke: None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            opacity: 1.0,
            visibility: false,
        };
        assert!(resolve_visibility(&el, &ctx));
    }

    // ----------------------------------------
    // resolve_opacity テスト
    // ----------------------------------------

    #[test]
    fn test_resolve_opacity_inline_over_attribute() {
        let el = make_element(&[("opacity", "0.3"), ("style", "opacity: 0.7")]);
        let ctx = StyleContext::default_root();
        let op = resolve_opacity(&el, &ctx);
        assert!((op - 0.7).abs() < f32::EPSILON);
    }
}

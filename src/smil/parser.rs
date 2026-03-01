use crate::svg::color::parse_color;
use crate::types::*;
use crate::xml::XmlElement;

/// XML ツリー全体を走査し、SMIL アニメーション要素を収集してパース済みリストを返す。
pub fn parse_smil_animations(root: &XmlElement) -> Vec<SmilAnimation> {
    let mut animations = Vec::new();
    collect_animations(root, None, &mut animations);
    animations
}

// ============================================
// ツリー走査
// ============================================

/// 再帰的にツリーを走査し、SMIL 要素 (<animate>, <set>, <animateTransform>) を見つけて
/// パースした SmilAnimation を結果リストに追加する。
///
/// parent_id: 親要素の id 属性値。SMIL 要素が子要素として配置されている場合に
///            ターゲット ID として使用する。
fn collect_animations(
    element: &XmlElement,
    parent_id: Option<&str>,
    out: &mut Vec<SmilAnimation>,
) {
    // 現在の要素が SMIL 要素かどうかを判定
    let smil_kind = match element.tag.as_str() {
        "animate" => Some(SmilKind::Animate),
        "set" => Some(SmilKind::Set),
        "animateTransform" => Some(SmilKind::AnimateTransform),
        _ => None,
    };

    if let Some(kind) = smil_kind {
        // ターゲット ID を解決
        let target_id = resolve_target_id(element, parent_id);
        if let Some(anim) = parse_single_animation(element, kind, &target_id) {
            out.push(anim);
        }
    }

    // 子要素を再帰走査。現在の要素の id を親 ID として渡す。
    let current_id = element.attribute("id");
    for child in element.children_elements() {
        collect_animations(child, current_id, out);
    }
}

// ============================================
// ターゲット ID 解決
// ============================================

/// SMIL 要素のターゲット要素 ID を解決する。
///
/// 優先順位:
/// 1. href 属性 (名前空間除去済みなので xlink:href も href として取得可能) の '#' を除去
/// 2. 親要素の id 属性
/// 3. 自動生成 ID (parse.rs で __smil_target_N として割り当て済み)
fn resolve_target_id(smil_elem: &XmlElement, parent_id: Option<&str>) -> String {
    // href 属性からターゲットを取得
    if let Some(href) = smil_elem.attribute("href") {
        let href = href.trim();
        if !href.is_empty() {
            return href.strip_prefix('#').unwrap_or(href).to_string();
        }
    }

    // 親要素の id を使用
    if let Some(id) = parent_id {
        return id.to_string();
    }

    // フォールバック: 空文字列 (通常は parse.rs が自動 ID を割り当て済み)
    String::new()
}

// ============================================
// 個別アニメーションのパース
// ============================================

/// 単一の SMIL 要素をパースして SmilAnimation を生成する。
/// attributeName が欠如している場合は None を返す (set 以外)。
fn parse_single_animation(
    elem: &XmlElement,
    kind: SmilKind,
    target_id: &str,
) -> Option<SmilAnimation> {
    let attribute = elem.attribute("attributeName").unwrap_or("").to_string();

    // animateTransform の type 属性
    let transform_type = if kind == SmilKind::AnimateTransform {
        Some(parse_transform_type(
            elem.attribute("type").unwrap_or("translate"),
        ))
    } else {
        None
    };

    // 値のパース
    let values = parse_values(elem, &attribute, &kind, transform_type.as_ref());

    // タイミング情報
    let timing = SmilTiming {
        begin: parse_duration(elem.attribute("begin").unwrap_or("0")),
        duration: parse_duration(elem.attribute("dur").unwrap_or("0")),
        repeat_count: parse_repeat_count(elem.attribute("repeatCount").unwrap_or("1")),
    };

    // calcMode
    let calc_mode = match elem.attribute("calcMode").unwrap_or("linear") {
        "discrete" => CalcMode::Discrete,
        "spline" => CalcMode::Spline,
        "paced" => CalcMode::Paced,
        _ => CalcMode::Linear,
    };

    // additive
    let additive = match elem.attribute("additive").unwrap_or("replace") {
        "sum" => Additive::Sum,
        _ => Additive::Replace,
    };

    // fill (アニメーション fill。SVG の fill 属性とは別物)
    let fill_mode = match elem.attribute("fill").unwrap_or("remove") {
        "freeze" => FillMode::Freeze,
        _ => FillMode::Remove,
    };

    // keyTimes
    let key_times = elem.attribute("keyTimes").map(parse_key_times);

    // keySplines
    let key_splines = elem.attribute("keySplines").map(parse_key_splines);

    Some(SmilAnimation {
        target_id: target_id.to_string(),
        kind,
        attribute,
        values,
        key_times,
        key_splines,
        timing,
        calc_mode,
        additive,
        fill_mode,
        transform_type,
    })
}

// ============================================
// 時間パーサー
// ============================================

/// SVG/SMIL の時間値文字列を秒数 (f64) にパースする。
///
/// 対応フォーマット:
/// - "3s"    → 3.0
/// - "500ms" → 0.5
/// - "2.5"   → 2.5 (単位なしは秒として解釈)
fn parse_duration(s: &str) -> f64 {
    let s = s.trim();

    if let Some(ms_str) = s.strip_suffix("ms") {
        // ミリ秒 → 秒に変換
        ms_str.trim().parse::<f64>().unwrap_or(0.0) / 1000.0
    } else if let Some(sec_str) = s.strip_suffix('s') {
        // 秒
        sec_str.trim().parse::<f64>().unwrap_or(0.0)
    } else {
        // 単位なし → 秒として解釈
        s.parse::<f64>().unwrap_or(0.0)
    }
}

// ============================================
// repeatCount パーサー
// ============================================

/// repeatCount 属性をパースする。
/// "indefinite" → Indefinite, 数値 → Definite(n)
fn parse_repeat_count(s: &str) -> RepeatCount {
    let s = s.trim();
    if s == "indefinite" {
        RepeatCount::Indefinite
    } else {
        RepeatCount::Definite(s.parse::<f64>().unwrap_or(1.0))
    }
}

// ============================================
// TransformType パーサー
// ============================================

/// animateTransform の type 属性をパースする。
fn parse_transform_type(s: &str) -> TransformType {
    match s.trim() {
        "rotate" => TransformType::Rotate,
        "scale" => TransformType::Scale,
        "skewX" => TransformType::SkewX,
        "skewY" => TransformType::SkewY,
        // "translate" およびデフォルト
        _ => TransformType::Translate,
    }
}

// ============================================
// Values パーサー
// ============================================

/// SMIL 要素の値リストをパースする。
///
/// 取得順序:
/// 1. values 属性: ';' で分割して各値をパース
/// 2. from + to: 2 要素のリスト
/// 3. from + by: from の値に by を加算した 2 要素のリスト (数値のみ)
/// 4. to のみ: 1 要素のリスト
fn parse_values(
    elem: &XmlElement,
    attribute: &str,
    kind: &SmilKind,
    transform_type: Option<&TransformType>,
) -> Vec<SmilValue> {
    // values 属性があればそれを使う
    if let Some(values_str) = elem.attribute("values") {
        return values_str
            .split(';')
            .map(|v| parse_smil_value(v.trim(), attribute, kind, transform_type))
            .collect();
    }

    let from = elem.attribute("from");
    let to = elem.attribute("to");
    let by = elem.attribute("by");

    match (from, to, by) {
        // from + to
        (Some(f), Some(t), _) => {
            vec![
                parse_smil_value(f.trim(), attribute, kind, transform_type),
                parse_smil_value(t.trim(), attribute, kind, transform_type),
            ]
        }
        // from + by (数値の加算)
        (Some(f), None, Some(b)) => {
            let from_val = parse_smil_value(f.trim(), attribute, kind, transform_type);
            let by_val = parse_smil_value(b.trim(), attribute, kind, transform_type);
            let to_val = add_smil_values(&from_val, &by_val);
            vec![from_val, to_val]
        }
        // to のみ
        (None, Some(t), _) => {
            vec![parse_smil_value(t.trim(), attribute, kind, transform_type)]
        }
        // 値が指定されていない
        _ => Vec::new(),
    }
}

/// 2 つの SmilValue を加算する (from + by の計算用)。
/// 数値型のみ加算し、Color はそのまま by を返す。
fn add_smil_values(a: &SmilValue, b: &SmilValue) -> SmilValue {
    match (a, b) {
        (SmilValue::Number(a), SmilValue::Number(b)) => SmilValue::Number(a + b),
        (SmilValue::NumberPair(a1, a2), SmilValue::NumberPair(b1, b2)) => {
            SmilValue::NumberPair(a1 + b1, a2 + b2)
        }
        (SmilValue::NumberTriple(a1, a2, a3), SmilValue::NumberTriple(b1, b2, b3)) => {
            SmilValue::NumberTriple(a1 + b1, a2 + b2, a3 + b3)
        }
        // 型が合わない場合や Color の場合は b をクローンして返す
        _ => b.clone(),
    }
}

// ============================================
// SmilValue パーサー
// ============================================

/// 属性名とコンテキストに基づいて、値文字列を適切な SmilValue にパースする。
///
/// パース戦略:
/// - animateTransform + translate → NumberPair
/// - animateTransform + scale    → Number or NumberPair
/// - animateTransform + rotate   → Number or NumberTriple
/// - fill/stroke (色属性)        → Color
/// - その他 (opacity 等)         → Number
fn parse_smil_value(
    s: &str,
    attribute: &str,
    kind: &SmilKind,
    transform_type: Option<&TransformType>,
) -> SmilValue {
    let s = s.trim();

    // animateTransform の場合は transform_type に応じてパース
    if *kind == SmilKind::AnimateTransform {
        if let Some(tt) = transform_type {
            return parse_transform_value(s, tt);
        }
    }

    // 色属性かどうかを判定
    if is_color_attribute(attribute) {
        if let Some(color) = parse_color(s) {
            return SmilValue::Color(color);
        }
    }

    // デフォルト: 数値としてパース
    SmilValue::Number(s.parse::<f64>().unwrap_or(0.0))
}

/// 属性名が色を表すかどうかを判定する。
fn is_color_attribute(attr: &str) -> bool {
    matches!(attr, "fill" | "stroke" | "color" | "stop-color" | "flood-color" | "lighting-color")
}

/// animateTransform の値を transform type に応じてパースする。
///
/// - translate: "10 20" → NumberPair(10, 20), "10" → NumberPair(10, 0)
/// - scale:     "2 3"   → NumberPair(2, 3),   "2"  → Number(2)
/// - rotate:    "45 10 20" → NumberTriple(45, 10, 20), "45" → Number(45)
/// - skewX/skewY: "30" → Number(30)
fn parse_transform_value(s: &str, tt: &TransformType) -> SmilValue {
    let nums = parse_number_list(s);

    match tt {
        TransformType::Translate => match nums.len() {
            0 => SmilValue::NumberPair(0.0, 0.0),
            1 => SmilValue::NumberPair(nums[0], 0.0),
            _ => SmilValue::NumberPair(nums[0], nums[1]),
        },
        TransformType::Scale => match nums.len() {
            0 => SmilValue::Number(1.0),
            1 => SmilValue::Number(nums[0]),
            _ => SmilValue::NumberPair(nums[0], nums[1]),
        },
        TransformType::Rotate => match nums.len() {
            0 => SmilValue::Number(0.0),
            1 => SmilValue::Number(nums[0]),
            2 => SmilValue::NumberTriple(nums[0], nums[1], 0.0),
            _ => SmilValue::NumberTriple(nums[0], nums[1], nums[2]),
        },
        TransformType::SkewX | TransformType::SkewY => {
            SmilValue::Number(nums.first().copied().unwrap_or(0.0))
        }
    }
}

/// スペースまたはカンマで区切られた数値リストをパースする。
fn parse_number_list(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ' ' || c == ',' || c == '\t')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse::<f64>().ok())
        .collect()
}

// ============================================
// keyTimes パーサー
// ============================================

/// keyTimes 属性をパースする。';' で区切られた f64 値のリスト。
/// 例: "0; 0.15; 0.45; 1" → [0.0, 0.15, 0.45, 1.0]
fn parse_key_times(s: &str) -> Vec<f64> {
    s.split(';')
        .map(|v| v.trim().parse::<f64>().unwrap_or(0.0))
        .collect()
}

// ============================================
// keySplines パーサー
// ============================================

/// keySplines 属性をパースする。
/// ';' でセグメントを区切り、各セグメントは 4 つの数値 (x1 y1 x2 y2) を含む。
/// 数値はスペースまたはカンマで区切られる。
///
/// 例: "0 0 0.58 1; 0.42 0 1 1" → [CubicBezier1D { x1:0, y1:0, x2:0.58, y2:1 }, ...]
fn parse_key_splines(s: &str) -> Vec<CubicBezier1D> {
    s.split(';')
        .filter_map(|segment| {
            let nums = parse_number_list(segment.trim());
            if nums.len() == 4 {
                Some(CubicBezier1D {
                    x1: nums[0],
                    y1: nums[1],
                    x2: nums[2],
                    y2: nums[3],
                })
            } else {
                None
            }
        })
        .collect()
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::{XmlElement, XmlNode};

    // ----------------------------------------
    // ヘルパー: テスト用の XmlElement を簡易生成
    // ----------------------------------------

    fn make_elem(tag: &str, attrs: &[(&str, &str)]) -> XmlElement {
        XmlElement {
            tag: tag.to_string(),
            attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            children: Vec::new(),
        }
    }

    fn make_elem_with_children(
        tag: &str,
        attrs: &[(&str, &str)],
        children: Vec<XmlElement>,
    ) -> XmlElement {
        XmlElement {
            tag: tag.to_string(),
            attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            children: children.into_iter().map(XmlNode::Element).collect(),
        }
    }

    // ========================================
    // parse_duration テスト
    // ========================================

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("3s"), 3.0);
    }

    #[test]
    fn test_parse_duration_milliseconds() {
        assert_eq!(parse_duration("500ms"), 0.5);
    }

    #[test]
    fn test_parse_duration_unitless() {
        assert_eq!(parse_duration("2.5"), 2.5);
    }

    #[test]
    fn test_parse_duration_zero() {
        assert_eq!(parse_duration("0"), 0.0);
    }

    #[test]
    fn test_parse_duration_fractional_seconds() {
        assert_eq!(parse_duration("1.5s"), 1.5);
    }

    #[test]
    fn test_parse_duration_large_ms() {
        assert_eq!(parse_duration("2000ms"), 2.0);
    }

    // ========================================
    // parse_smil_value テスト
    // ========================================

    #[test]
    fn test_parse_smil_value_number() {
        let val = parse_smil_value("0.5", "opacity", &SmilKind::Animate, None);
        assert_eq!(val, SmilValue::Number(0.5));
    }

    #[test]
    fn test_parse_smil_value_number_negative() {
        let val = parse_smil_value("-10", "stroke-dashoffset", &SmilKind::Animate, None);
        assert_eq!(val, SmilValue::Number(-10.0));
    }

    #[test]
    fn test_parse_smil_value_translate_pair() {
        let val = parse_smil_value(
            "10 20",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Translate),
        );
        assert_eq!(val, SmilValue::NumberPair(10.0, 20.0));
    }

    #[test]
    fn test_parse_smil_value_translate_single() {
        // translate with single value → NumberPair(x, 0)
        let val = parse_smil_value(
            "15",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Translate),
        );
        assert_eq!(val, SmilValue::NumberPair(15.0, 0.0));
    }

    #[test]
    fn test_parse_smil_value_rotate_single() {
        let val = parse_smil_value(
            "45",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Rotate),
        );
        assert_eq!(val, SmilValue::Number(45.0));
    }

    #[test]
    fn test_parse_smil_value_rotate_triple() {
        let val = parse_smil_value(
            "45 50 50",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Rotate),
        );
        assert_eq!(val, SmilValue::NumberTriple(45.0, 50.0, 50.0));
    }

    #[test]
    fn test_parse_smil_value_scale_single() {
        let val = parse_smil_value(
            "2",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Scale),
        );
        assert_eq!(val, SmilValue::Number(2.0));
    }

    #[test]
    fn test_parse_smil_value_scale_pair() {
        let val = parse_smil_value(
            "2 3",
            "transform",
            &SmilKind::AnimateTransform,
            Some(&TransformType::Scale),
        );
        assert_eq!(val, SmilValue::NumberPair(2.0, 3.0));
    }

    #[test]
    fn test_parse_smil_value_color() {
        let val = parse_smil_value("#FF0000", "fill", &SmilKind::Animate, None);
        assert_eq!(val, SmilValue::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
    }

    #[test]
    fn test_parse_smil_value_named_color() {
        let val = parse_smil_value("red", "stroke", &SmilKind::Animate, None);
        assert_eq!(val, SmilValue::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
    }

    // ========================================
    // 単純な animate 要素のパーステスト
    // ========================================

    #[test]
    fn test_parse_simple_animate() {
        // <g id="target1"><animate attributeName="opacity" from="0" to="1" dur="2s" /></g>
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "2s"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "target1")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);

        let anim = &animations[0];
        assert_eq!(anim.target_id, "target1");
        assert_eq!(anim.kind, SmilKind::Animate);
        assert_eq!(anim.attribute, "opacity");
        assert_eq!(anim.values, vec![SmilValue::Number(0.0), SmilValue::Number(1.0)]);
        assert_eq!(anim.timing.duration, 2.0);
        assert_eq!(anim.timing.begin, 0.0);
        assert_eq!(anim.calc_mode, CalcMode::Linear);
        assert_eq!(anim.additive, Additive::Replace);
        assert_eq!(anim.fill_mode, FillMode::Remove);
        assert!(anim.transform_type.is_none());
    }

    // ========================================
    // animateTransform テスト
    // ========================================

    #[test]
    fn test_parse_animate_transform_scale() {
        // checkout.svg のような animateTransform
        let elem = make_elem("animateTransform", &[
            ("attributeName", "transform"),
            ("type", "scale"),
            ("values", "0; 1; 1; 0; 0; 1; 1"),
            ("keyTimes", "0; 0.15; 0.45; 0.52; 0.90; 0.95; 1"),
            ("dur", "3s"),
            ("repeatCount", "indefinite"),
            ("calcMode", "spline"),
            ("keySplines", "0 0 0.58 1; 0 0 1 1; 0.42 0 0.58 1; 0 0 1 1; 0 0 1 1; 0 0 1 1"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "anim-group")], vec![elem]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);

        let anim = &animations[0];
        assert_eq!(anim.kind, SmilKind::AnimateTransform);
        assert_eq!(anim.transform_type, Some(TransformType::Scale));
        assert_eq!(anim.values.len(), 7);
        // scale with single value → Number
        assert_eq!(anim.values[0], SmilValue::Number(0.0));
        assert_eq!(anim.values[1], SmilValue::Number(1.0));
        assert_eq!(anim.timing.duration, 3.0);
        assert_eq!(anim.timing.repeat_count, RepeatCount::Indefinite);
        assert_eq!(anim.calc_mode, CalcMode::Spline);

        // keySplines: 6 個
        let splines = anim.key_splines.as_ref().unwrap();
        assert_eq!(splines.len(), 6);
        assert_eq!(
            splines[0],
            CubicBezier1D { x1: 0.0, y1: 0.0, x2: 0.58, y2: 1.0 }
        );
        assert_eq!(
            splines[2],
            CubicBezier1D { x1: 0.42, y1: 0.0, x2: 0.58, y2: 1.0 }
        );
    }

    // ========================================
    // keyTimes テスト
    // ========================================

    #[test]
    fn test_parse_key_times() {
        let result = parse_key_times("0; 0.15; 0.45; 0.52; 0.90; 0.95; 1");
        assert_eq!(result.len(), 7);
        assert_eq!(result[0], 0.0);
        assert_eq!(result[1], 0.15);
        assert_eq!(result[6], 1.0);
    }

    #[test]
    fn test_parse_key_times_simple() {
        let result = parse_key_times("0; 1");
        assert_eq!(result, vec![0.0, 1.0]);
    }

    // ========================================
    // keySplines テスト
    // ========================================

    #[test]
    fn test_parse_key_splines() {
        let result = parse_key_splines("0 0 0.58 1; 0.42 0 1 1");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], CubicBezier1D { x1: 0.0, y1: 0.0, x2: 0.58, y2: 1.0 });
        assert_eq!(result[1], CubicBezier1D { x1: 0.42, y1: 0.0, x2: 1.0, y2: 1.0 });
    }

    #[test]
    fn test_parse_key_splines_with_commas() {
        let result = parse_key_splines("0,0,0.58,1; 0.42,0,1,1");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], CubicBezier1D { x1: 0.0, y1: 0.0, x2: 0.58, y2: 1.0 });
    }

    // ========================================
    // ターゲット ID テスト
    // ========================================

    #[test]
    fn test_target_id_from_parent() {
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
        ]);
        let parent = make_elem_with_children("rect", &[("id", "my-rect")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].target_id, "my-rect");
    }

    #[test]
    fn test_target_id_from_href() {
        let animate = make_elem("animate", &[
            ("href", "#external-target"),
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "parent-id")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        // href が優先される
        assert_eq!(animations[0].target_id, "external-target");
    }

    #[test]
    fn test_target_id_auto_generated() {
        // 親に id がなく、href もない場合 → 空文字列
        // (実際には parse.rs が __smil_target_N を割り当て済み)
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
        ]);
        let parent = make_elem_with_children("g", &[], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].target_id, "");
    }

    // ========================================
    // repeatCount テスト
    // ========================================

    #[test]
    fn test_repeat_count_indefinite() {
        assert_eq!(parse_repeat_count("indefinite"), RepeatCount::Indefinite);
    }

    #[test]
    fn test_repeat_count_numeric() {
        assert_eq!(parse_repeat_count("3"), RepeatCount::Definite(3.0));
    }

    #[test]
    fn test_repeat_count_fractional() {
        assert_eq!(parse_repeat_count("2.5"), RepeatCount::Definite(2.5));
    }

    #[test]
    fn test_repeat_count_default() {
        assert_eq!(parse_repeat_count("1"), RepeatCount::Definite(1.0));
    }

    // ========================================
    // set 要素テスト
    // ========================================

    #[test]
    fn test_parse_set_element() {
        let set = make_elem("set", &[
            ("attributeName", "fill"),
            ("to", "#FF0000"),
            ("begin", "1s"),
            ("dur", "2s"),
            ("fill", "freeze"),
        ]);
        let parent = make_elem_with_children("rect", &[("id", "rect1")], vec![set]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);

        let anim = &animations[0];
        assert_eq!(anim.kind, SmilKind::Set);
        assert_eq!(anim.target_id, "rect1");
        assert_eq!(anim.attribute, "fill");
        assert_eq!(anim.values, vec![SmilValue::Color(Color { r: 255, g: 0, b: 0, a: 255 })]);
        assert_eq!(anim.timing.begin, 1.0);
        assert_eq!(anim.timing.duration, 2.0);
        assert_eq!(anim.fill_mode, FillMode::Freeze);
    }

    // ========================================
    // values 属性からのパーステスト
    // ========================================

    #[test]
    fn test_parse_values_attribute() {
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("values", "0; 1; 1; 0"),
            ("dur", "3s"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "g1")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        assert_eq!(
            animations[0].values,
            vec![
                SmilValue::Number(0.0),
                SmilValue::Number(1.0),
                SmilValue::Number(1.0),
                SmilValue::Number(0.0),
            ]
        );
    }

    // ========================================
    // from + by テスト
    // ========================================

    #[test]
    fn test_parse_from_by() {
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0.2"),
            ("by", "0.3"),
            ("dur", "1s"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "g1")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].values[0], SmilValue::Number(0.2));
        // 0.2 + 0.3 = 0.5 (浮動小数点の近似)
        if let SmilValue::Number(v) = animations[0].values[1] {
            assert!((v - 0.5).abs() < 1e-10);
        } else {
            panic!("Expected Number");
        }
    }

    // ========================================
    // 複数アニメーション要素テスト
    // ========================================

    #[test]
    fn test_parse_multiple_animations() {
        let animate1 = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
        ]);
        let animate2 = make_elem("animateTransform", &[
            ("attributeName", "transform"),
            ("type", "rotate"),
            ("from", "0"),
            ("to", "360"),
            ("dur", "2s"),
            ("repeatCount", "indefinite"),
        ]);
        let parent = make_elem_with_children(
            "g",
            &[("id", "animated")],
            vec![animate1, animate2],
        );
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 2);

        assert_eq!(animations[0].kind, SmilKind::Animate);
        assert_eq!(animations[0].attribute, "opacity");

        assert_eq!(animations[1].kind, SmilKind::AnimateTransform);
        assert_eq!(animations[1].attribute, "transform");
        assert_eq!(animations[1].transform_type, Some(TransformType::Rotate));
        assert_eq!(animations[1].timing.repeat_count, RepeatCount::Indefinite);
    }

    // ========================================
    // additive=sum テスト
    // ========================================

    #[test]
    fn test_additive_sum() {
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
            ("additive", "sum"),
        ]);
        let parent = make_elem_with_children("g", &[("id", "g1")], vec![animate]);
        let root = make_elem_with_children("svg", &[], vec![parent]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations[0].additive, Additive::Sum);
    }

    // ========================================
    // 深いネストのツリー走査テスト
    // ========================================

    #[test]
    fn test_deep_nested_tree_walk() {
        let animate = make_elem("animate", &[
            ("attributeName", "opacity"),
            ("from", "0"),
            ("to", "1"),
            ("dur", "1s"),
        ]);
        let inner = make_elem_with_children("g", &[("id", "inner")], vec![animate]);
        let outer = make_elem_with_children("g", &[("id", "outer")], vec![inner]);
        let root = make_elem_with_children("svg", &[], vec![outer]);

        let animations = parse_smil_animations(&root);
        assert_eq!(animations.len(), 1);
        assert_eq!(animations[0].target_id, "inner");
    }
}

use crate::types::*;
use crate::xml;
use crate::svg as svg_mod;

// SMIL アニメーション要素のタグ名一覧。
// これらの要素は SMIL パーサーで別途処理されるため、シーンツリー構築時にはスキップする。
const SMIL_TAGS: &[&str] = &["animate", "set", "animateTransform", "animateMotion"];

// ジオメトリ要素のタグ名一覧。
// convert_element がこれらのタグに対して Vec<PathSegment> を返す。
const GEOMETRY_TAGS: &[&str] = &[
    "rect", "circle", "ellipse", "line", "polyline", "polygon", "path",
];

/// SVG バイト列をパースして SvgDocument を構築する。
///
/// 処理フロー:
/// 1. XML パーサーでバイト列を XmlDocument に変換
/// 2. ルート要素が <svg> であることを確認し、viewBox をパース
/// 3. SMIL アニメーション子要素を持つが id を持たない要素に自動 ID を付与
/// 4. XML ツリーを再帰的に走査してシーンツリー (SvgGroup / SvgPath) を構築
/// 5. SMIL アニメーション情報を取得 (現在はスタブのため空 Vec)
pub fn parse(data: &[u8]) -> Result<SvgDocument, SvgError> {
    // 1. XML パース
    let mut xml_doc = xml::parse_xml(data).map_err(SvgError::XmlParse)?;

    // 2. ルート要素の検証
    if xml_doc.root.tag != "svg" {
        return Err(SvgError::InvalidSvg(
            "Root element is not <svg>".into(),
        ));
    }

    // 3. viewBox のパース
    let view_box = parse_view_box(&xml_doc.root)?;

    // 4. SMIL ターゲット要素への自動 ID 付与
    //    SMIL アニメーション子要素 (animate, set, animateTransform) を持つが
    //    id 属性を持たない要素に __smil_target_N 形式の ID を自動生成する。
    let mut auto_id_counter: u32 = 0;
    assign_auto_ids(&mut xml_doc.root, &mut auto_id_counter);

    // 5. シーンツリーの構築
    let root = build_group(&xml_doc.root);

    // 6. SMIL アニメーションの取得
    let animations = crate::smil::parser::parse_smil_animations(&xml_doc.root);

    Ok(SvgDocument {
        view_box,
        root,
        animations,
    })
}

/// <svg> 要素から ViewBox をパースする。
///
/// 解決順序:
/// 1. viewBox 属性がある場合: "x y width height" フォーマットでパース
/// 2. viewBox がない場合: width/height 属性からフォールバック (x=0, y=0)
/// 3. どちらもない場合: エラー
fn parse_view_box(svg_el: &xml::XmlElement) -> Result<ViewBox, SvgError> {
    if let Some(vb_str) = svg_el.attribute("viewBox") {
        parse_view_box_str(vb_str)
    } else {
        // viewBox 属性がない場合、width/height からフォールバック
        let w = svg_el
            .attribute("width")
            .and_then(|s| parse_length_value(s));
        let h = svg_el
            .attribute("height")
            .and_then(|s| parse_length_value(s));

        match (w, h) {
            (Some(width), Some(height)) => Ok(ViewBox {
                x: 0.0,
                y: 0.0,
                width,
                height,
            }),
            _ => Err(SvgError::InvalidSvg(
                "No viewBox attribute and no valid width/height on <svg>".into(),
            )),
        }
    }
}

/// "x y width height" 形式の viewBox 文字列をパースする。
/// 区切り文字としてカンマ・空白の両方を許容する。
fn parse_view_box_str(s: &str) -> Result<ViewBox, SvgError> {
    let nums: Vec<f32> = s
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<f32>().map_err(|_| {
                SvgError::InvalidSvg(format!("Invalid number in viewBox: '{}'", t))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if nums.len() != 4 {
        return Err(SvgError::InvalidSvg(format!(
            "viewBox requires exactly 4 values, got {}",
            nums.len()
        )));
    }

    Ok(ViewBox {
        x: nums[0],
        y: nums[1],
        width: nums[2],
        height: nums[3],
    })
}

/// 長さ属性値から数値部分を抽出する。
/// "100px", "200", "50%" のような入力から数値部分のみを取り出す。
/// パース不能な場合は None を返す。
fn parse_length_value(s: &str) -> Option<f32> {
    let s = s.trim();
    // 末尾の単位サフィックス (px, em, %, etc.) を除去して数値部分を取得
    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+')
        .unwrap_or(s.len());
    s[..num_end].parse::<f32>().ok()
}

/// SMIL アニメーション子要素を持つが id 属性を持たない要素に自動 ID を付与する。
///
/// 再帰的に XML ツリーを走査し、以下の条件を満たす要素に `__smil_target_N` を割り当てる:
/// - id 属性が未設定
/// - 直接の子要素に SMIL タグ (animate, set, animateTransform, animateMotion) がある
fn assign_auto_ids(el: &mut xml::XmlElement, counter: &mut u32) {
    // この要素自身に SMIL 子要素があり、かつ id が未設定なら自動 ID を付与
    let has_smil_child = el.children.iter().any(|child| {
        matches!(child, xml::XmlNode::Element(e) if SMIL_TAGS.contains(&e.tag.as_str()))
    });

    if has_smil_child && el.attribute("id").is_none() {
        let auto_id = format!("__smil_target_{}", counter);
        el.attrs.push(("id".to_string(), auto_id));
        *counter += 1;
    }

    // 子要素にも再帰的に適用
    for child in &mut el.children {
        if let xml::XmlNode::Element(child_el) = child {
            assign_auto_ids(child_el, counter);
        }
    }
}

/// <svg> または <g> 要素の子要素群を再帰的に走査して SvgGroup を構築する。
///
/// 処理ルール:
/// - <g> → SvgGroup (transform, opacity を保持し、子要素を再帰処理)
/// - ジオメトリ要素 (rect, circle, ellipse, line, polyline, polygon, path)
///   → convert_element で PathSegment 列に変換し SvgPath を生成
/// - SMIL 要素 (animate, set, animateTransform) → スキップ (SMIL パーサーで処理)
/// - その他の要素 (defs, use, text 等) → 要素自体はスキップするが子要素は再帰処理
///   (ジオメトリ要素がネストされている可能性があるため)
fn build_group(el: &xml::XmlElement) -> SvgGroup {
    let id = el.attribute("id").map(|s| s.to_string());
    let transform = el
        .attribute("transform")
        .map(|s| svg_mod::transform::parse_transform(s))
        .unwrap_or_else(Affine2D::identity);
    let opacity = svg_mod::style::parse_opacity(el);

    let mut children = Vec::new();

    for child_el in el.children_elements() {
        process_element(child_el, &mut children);
    }

    SvgGroup {
        id,
        transform,
        opacity,
        children,
    }
}

/// 単一の XML 要素を処理し、結果を children リストに追加する。
fn process_element(el: &xml::XmlElement, children: &mut Vec<SvgNode>) {
    let tag = el.tag.as_str();

    // SMIL 要素はスキップ (SMIL パーサーで別途処理)
    if SMIL_TAGS.contains(&tag) {
        return;
    }

    // <g> 要素 → SvgGroup として再帰処理
    if tag == "g" {
        children.push(SvgNode::Group(build_group(el)));
        return;
    }

    // ジオメトリ要素 → SvgPath に変換
    if GEOMETRY_TAGS.contains(&tag) {
        if let Some(segments) = svg_mod::elements::convert_element(el) {
            let path = SvgPath {
                id: el.attribute("id").map(|s| s.to_string()),
                segments,
                fill: svg_mod::style::parse_fill(el),
                stroke: svg_mod::style::parse_stroke(el),
                opacity: svg_mod::style::parse_opacity(el),
            };
            children.push(SvgNode::Path(path));
        }
        return;
    }

    // その他の要素 → 要素自体はスキップするが、子要素にジオメトリが含まれる
    // 可能性があるため再帰的に処理する (例: <defs> 内の <g> や <clipPath> 内の図形)
    for child_el in el.children_elements() {
        process_element(child_el, children);
    }
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------
    // parse_view_box_str テスト
    // ----------------------------------------

    #[test]
    fn test_parse_view_box_str_standard() {
        let vb = parse_view_box_str("0 0 400 400").unwrap();
        assert_eq!(vb.x, 0.0);
        assert_eq!(vb.y, 0.0);
        assert_eq!(vb.width, 400.0);
        assert_eq!(vb.height, 400.0);
    }

    #[test]
    fn test_parse_view_box_str_with_commas() {
        let vb = parse_view_box_str("0,0,100,200").unwrap();
        assert_eq!(vb.x, 0.0);
        assert_eq!(vb.y, 0.0);
        assert_eq!(vb.width, 100.0);
        assert_eq!(vb.height, 200.0);
    }

    #[test]
    fn test_parse_view_box_str_negative_offset() {
        let vb = parse_view_box_str("-50 -50 200 200").unwrap();
        assert_eq!(vb.x, -50.0);
        assert_eq!(vb.y, -50.0);
        assert_eq!(vb.width, 200.0);
        assert_eq!(vb.height, 200.0);
    }

    #[test]
    fn test_parse_view_box_str_decimal() {
        let vb = parse_view_box_str("0.5 1.5 100.25 200.75").unwrap();
        assert_eq!(vb.x, 0.5);
        assert_eq!(vb.y, 1.5);
        assert_eq!(vb.width, 100.25);
        assert_eq!(vb.height, 200.75);
    }

    #[test]
    fn test_parse_view_box_str_too_few_values() {
        let result = parse_view_box_str("0 0 400");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_view_box_str_too_many_values() {
        let result = parse_view_box_str("0 0 400 400 500");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_view_box_str_invalid_number() {
        let result = parse_view_box_str("0 0 abc 400");
        assert!(result.is_err());
    }

    // ----------------------------------------
    // parse_length_value テスト
    // ----------------------------------------

    #[test]
    fn test_parse_length_plain_number() {
        assert_eq!(parse_length_value("100"), Some(100.0));
    }

    #[test]
    fn test_parse_length_with_px() {
        assert_eq!(parse_length_value("200px"), Some(200.0));
    }

    #[test]
    fn test_parse_length_with_percent() {
        assert_eq!(parse_length_value("50%"), Some(50.0));
    }

    #[test]
    fn test_parse_length_decimal() {
        assert_eq!(parse_length_value("12.5"), Some(12.5));
    }

    #[test]
    fn test_parse_length_empty() {
        assert_eq!(parse_length_value(""), None);
    }

    #[test]
    fn test_parse_length_non_numeric() {
        assert_eq!(parse_length_value("auto"), None);
    }

    // ----------------------------------------
    // parse_view_box テスト (viewBox 属性 vs width/height フォールバック)
    // ----------------------------------------

    #[test]
    fn test_parse_view_box_from_attribute() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![("viewBox".to_string(), "0 0 800 600".to_string())],
            children: Vec::new(),
        };
        let vb = parse_view_box(&el).unwrap();
        assert_eq!(vb.width, 800.0);
        assert_eq!(vb.height, 600.0);
    }

    #[test]
    fn test_parse_view_box_fallback_width_height() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![
                ("width".to_string(), "640".to_string()),
                ("height".to_string(), "480".to_string()),
            ],
            children: Vec::new(),
        };
        let vb = parse_view_box(&el).unwrap();
        assert_eq!(vb.x, 0.0);
        assert_eq!(vb.y, 0.0);
        assert_eq!(vb.width, 640.0);
        assert_eq!(vb.height, 480.0);
    }

    #[test]
    fn test_parse_view_box_fallback_width_height_with_units() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![
                ("width".to_string(), "640px".to_string()),
                ("height".to_string(), "480px".to_string()),
            ],
            children: Vec::new(),
        };
        let vb = parse_view_box(&el).unwrap();
        assert_eq!(vb.width, 640.0);
        assert_eq!(vb.height, 480.0);
    }

    #[test]
    fn test_parse_view_box_missing_all_returns_error() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: Vec::new(),
        };
        assert!(parse_view_box(&el).is_err());
    }

    // ----------------------------------------
    // assign_auto_ids テスト
    // ----------------------------------------

    #[test]
    fn test_assign_auto_ids_to_element_with_smil_child() {
        let mut el = xml::XmlElement {
            tag: "rect".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "animate".to_string(),
                attrs: vec![],
                children: Vec::new(),
            })],
        };

        let mut counter = 0;
        assign_auto_ids(&mut el, &mut counter);

        assert_eq!(el.attribute("id"), Some("__smil_target_0"));
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_assign_auto_ids_preserves_existing_id() {
        let mut el = xml::XmlElement {
            tag: "rect".to_string(),
            attrs: vec![("id".to_string(), "my-rect".to_string())],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "animate".to_string(),
                attrs: vec![],
                children: Vec::new(),
            })],
        };

        let mut counter = 0;
        assign_auto_ids(&mut el, &mut counter);

        // 既に id がある場合は上書きしない
        assert_eq!(el.attribute("id"), Some("my-rect"));
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_assign_auto_ids_no_smil_children() {
        let mut el = xml::XmlElement {
            tag: "rect".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "title".to_string(),
                attrs: vec![],
                children: Vec::new(),
            })],
        };

        let mut counter = 0;
        assign_auto_ids(&mut el, &mut counter);

        // SMIL 子要素がないので id は付与されない
        assert_eq!(el.attribute("id"), None);
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_assign_auto_ids_multiple_elements() {
        let mut el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![],
                    children: vec![xml::XmlNode::Element(xml::XmlElement {
                        tag: "animate".to_string(),
                        attrs: vec![],
                        children: Vec::new(),
                    })],
                }),
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "circle".to_string(),
                    attrs: vec![],
                    children: vec![xml::XmlNode::Element(xml::XmlElement {
                        tag: "set".to_string(),
                        attrs: vec![],
                        children: Vec::new(),
                    })],
                }),
            ],
        };

        let mut counter = 0;
        assign_auto_ids(&mut el, &mut counter);

        // svg 自体は SMIL 子要素を持たない (直接の子は rect, circle)
        // rect と circle にそれぞれ自動 ID が付与される
        if let xml::XmlNode::Element(ref rect) = el.children[0] {
            assert_eq!(rect.attribute("id"), Some("__smil_target_0"));
        }
        if let xml::XmlNode::Element(ref circle) = el.children[1] {
            assert_eq!(circle.attribute("id"), Some("__smil_target_1"));
        }
        assert_eq!(counter, 2);
    }

    // ----------------------------------------
    // build_group テスト
    // ----------------------------------------

    #[test]
    fn test_build_group_empty() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: Vec::new(),
        };
        let group = build_group(&el);
        assert!(group.children.is_empty());
        assert_eq!(group.transform, Affine2D::identity());
        assert_eq!(group.opacity, 1.0);
    }

    #[test]
    fn test_build_group_with_transform() {
        let el = xml::XmlElement {
            tag: "g".to_string(),
            attrs: vec![
                ("id".to_string(), "g1".to_string()),
                ("transform".to_string(), "translate(10, 20)".to_string()),
                ("opacity".to_string(), "0.5".to_string()),
            ],
            children: Vec::new(),
        };
        let group = build_group(&el);
        assert_eq!(group.id, Some("g1".to_string()));
        assert_eq!(group.transform, Affine2D::translate(10.0, 20.0));
        assert_eq!(group.opacity, 0.5);
    }

    #[test]
    fn test_build_group_with_rect_child() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "rect".to_string(),
                attrs: vec![
                    ("x".to_string(), "0".to_string()),
                    ("y".to_string(), "0".to_string()),
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                    ("fill".to_string(), "#FF0000".to_string()),
                ],
                children: Vec::new(),
            })],
        };

        let group = build_group(&el);
        assert_eq!(group.children.len(), 1);

        match &group.children[0] {
            SvgNode::Path(path) => {
                assert!(!path.segments.is_empty());
                assert!(path.fill.is_some());
                let fill = path.fill.as_ref().unwrap();
                assert_eq!(fill.color, Color { r: 255, g: 0, b: 0, a: 255 });
            }
            _ => panic!("Expected SvgNode::Path"),
        }
    }

    #[test]
    fn test_build_group_nested_g() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "g".to_string(),
                attrs: vec![("id".to_string(), "outer".to_string())],
                children: vec![xml::XmlNode::Element(xml::XmlElement {
                    tag: "g".to_string(),
                    attrs: vec![("id".to_string(), "inner".to_string())],
                    children: vec![xml::XmlNode::Element(xml::XmlElement {
                        tag: "circle".to_string(),
                        attrs: vec![
                            ("cx".to_string(), "50".to_string()),
                            ("cy".to_string(), "50".to_string()),
                            ("r".to_string(), "25".to_string()),
                        ],
                        children: Vec::new(),
                    })],
                })],
            })],
        };

        let group = build_group(&el);
        assert_eq!(group.children.len(), 1);

        match &group.children[0] {
            SvgNode::Group(outer) => {
                assert_eq!(outer.id, Some("outer".to_string()));
                assert_eq!(outer.children.len(), 1);
                match &outer.children[0] {
                    SvgNode::Group(inner) => {
                        assert_eq!(inner.id, Some("inner".to_string()));
                        assert_eq!(inner.children.len(), 1);
                        assert!(matches!(&inner.children[0], SvgNode::Path(_)));
                    }
                    _ => panic!("Expected inner SvgNode::Group"),
                }
            }
            _ => panic!("Expected outer SvgNode::Group"),
        }
    }

    #[test]
    fn test_build_group_skips_smil_elements() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![
                        ("width".to_string(), "100".to_string()),
                        ("height".to_string(), "50".to_string()),
                    ],
                    children: vec![
                        // rect の子に animate があってもシーンツリーには含まれない
                        xml::XmlNode::Element(xml::XmlElement {
                            tag: "animate".to_string(),
                            attrs: vec![],
                            children: Vec::new(),
                        }),
                    ],
                }),
                // トップレベルの set もスキップ
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "set".to_string(),
                    attrs: vec![],
                    children: Vec::new(),
                }),
            ],
        };

        let group = build_group(&el);
        // rect は含まれるが、set はスキップ
        assert_eq!(group.children.len(), 1);
        assert!(matches!(&group.children[0], SvgNode::Path(_)));
    }

    #[test]
    fn test_build_group_unknown_element_children_processed() {
        // <defs> のような不明要素内にある <g> のジオメトリ要素も処理される
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "defs".to_string(),
                attrs: vec![],
                children: vec![xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![
                        ("width".to_string(), "50".to_string()),
                        ("height".to_string(), "50".to_string()),
                    ],
                    children: Vec::new(),
                })],
            })],
        };

        let group = build_group(&el);
        // defs 自体はスキップされるが、中の rect は処理される
        assert_eq!(group.children.len(), 1);
        assert!(matches!(&group.children[0], SvgNode::Path(_)));
    }

    // ----------------------------------------
    // parse (統合) テスト
    // ----------------------------------------

    #[test]
    fn test_parse_minimal_svg() {
        let svg = br#"<svg viewBox="0 0 100 100"></svg>"#;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.view_box.width, 100.0);
        assert_eq!(doc.view_box.height, 100.0);
        assert!(doc.root.children.is_empty());
        assert!(doc.animations.is_empty());
    }

    #[test]
    fn test_parse_svg_with_geometry() {
        let svg = br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 400">
  <g id="group1" transform="translate(10,20)">
    <rect x="0" y="0" width="100" height="50" fill="#FF0000"/>
    <circle cx="50" cy="25" r="10"/>
  </g>
  <path d="M0 0L10 10Z" stroke="#000000"/>
</svg>"##;

        let doc = parse(svg).unwrap();
        assert_eq!(doc.view_box, ViewBox { x: 0.0, y: 0.0, width: 400.0, height: 400.0 });

        // ルートの子: <g> と <path>
        assert_eq!(doc.root.children.len(), 2);

        // <g>
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert_eq!(g.id, Some("group1".to_string()));
                assert_eq!(g.transform, Affine2D::translate(10.0, 20.0));
                // rect + circle = 2 children
                assert_eq!(g.children.len(), 2);
            }
            _ => panic!("Expected Group"),
        }

        // <path>
        match &doc.root.children[1] {
            SvgNode::Path(p) => {
                assert_eq!(p.segments.len(), 3); // M, L, Z
                assert!(p.stroke.is_some());
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_parse_svg_with_width_height_fallback() {
        let svg = br#"<svg width="640" height="480"><rect width="100" height="50"/></svg>"#;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.view_box, ViewBox { x: 0.0, y: 0.0, width: 640.0, height: 480.0 });
    }

    #[test]
    fn test_parse_non_svg_root_returns_error() {
        let svg = b"<div></div>";
        let result = parse(svg);
        assert!(result.is_err());
        match result.unwrap_err() {
            SvgError::InvalidSvg(msg) => assert!(msg.contains("not <svg>")),
            other => panic!("Expected InvalidSvg, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_invalid_xml_returns_error() {
        let svg = b"<<<not xml>>>";
        let result = parse(svg);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SvgError::XmlParse(_)));
    }

    #[test]
    fn test_parse_no_viewbox_no_dimensions_returns_error() {
        let svg = b"<svg></svg>";
        let result = parse(svg);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SvgError::InvalidSvg(_)));
    }

    #[test]
    fn test_parse_preserves_element_ids() {
        let svg = br#"<svg viewBox="0 0 100 100">
            <rect id="my-rect" width="10" height="10"/>
        </svg>"#;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => assert_eq!(p.id, Some("my-rect".to_string())),
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_parse_opacity_propagation() {
        let svg = br#"<svg viewBox="0 0 100 100">
            <g opacity="0.5">
                <rect width="10" height="10" opacity="0.8"/>
            </g>
        </svg>"#;
        let doc = parse(svg).unwrap();

        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert_eq!(g.opacity, 0.5);
                match &g.children[0] {
                    SvgNode::Path(p) => assert_eq!(p.opacity, 0.8),
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_parse_svg_with_animations() {
        let svg = br##"<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <rect id="r1" width="50" height="50" fill="#FF0000">
                <animate attributeName="opacity" from="0" to="1" dur="2s" fill="freeze"/>
            </rect>
            <circle id="c1" cx="50" cy="50" r="25">
                <animateTransform attributeName="transform" type="scale" from="0" to="1" dur="1s" repeatCount="indefinite"/>
            </circle>
        </svg>"##;
        let doc = parse(svg).unwrap();
        // アニメーションが 2 つ取得されていることを確認
        assert_eq!(doc.animations.len(), 2);
        // ターゲット ID が正しいこと
        assert_eq!(doc.animations[0].target_id, "r1");
        assert_eq!(doc.animations[1].target_id, "c1");
        // 属性名が正しいこと
        assert_eq!(doc.animations[0].attribute, "opacity");
        assert_eq!(doc.animations[1].attribute, "transform");
    }
}

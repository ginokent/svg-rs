use std::collections::{HashMap, HashSet};

use crate::types::*;
use crate::xml;
use crate::svg as svg_mod;

/// defs テーブルのエントリ (パース内部データ)
enum DefEntry {
    /// <use> で参照される生の XML 要素 (rect, circle, g 等)
    Element(xml::XmlElement),
    /// <linearGradient> 要素
    LinearGradient(LinearGradient),
    /// <radialGradient> 要素
    RadialGradient(RadialGradient),
    /// <clipPath> 要素内のジオメトリを PathSegment 列に変換したもの
    ClipPath(Vec<PathSegment>),
}

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

    // 5. defs テーブルの構築
    let defs = collect_defs(&xml_doc.root);

    // 6. シーンツリーの構築
    let root_ctx = svg_mod::style::StyleContext::default_root();
    let root = build_group(&xml_doc.root, &defs, &mut HashSet::new(), &root_ctx);

    // 7. SMIL アニメーションの取得
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
            .and_then(parse_length_value);
        let h = svg_el
            .attribute("height")
            .and_then(parse_length_value);

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

/// <defs> および <symbol> 内の子要素を再帰的に収集し、id をキーとした定義テーブルを構築する。
fn collect_defs(root: &xml::XmlElement) -> HashMap<String, DefEntry> {
    let mut defs = HashMap::new();
    collect_defs_recursive(root, &mut defs);
    defs
}

fn collect_defs_recursive(el: &xml::XmlElement, defs: &mut HashMap<String, DefEntry>) {
    for child in el.children_elements() {
        if child.tag == "defs" || child.tag == "symbol" {
            // <symbol> 自体も id があれば登録する (<use> から参照される)
            if child.tag == "symbol" {
                if let Some(id) = child.attribute("id") {
                    defs.insert(id.to_string(), DefEntry::Element(child.clone()));
                }
            }
            // <defs>/<symbol> の直接の子要素をタグに応じて登録
            for grandchild in child.children_elements() {
                match grandchild.tag.as_str() {
                    "linearGradient" => {
                        if let Some(id) = grandchild.attribute("id") {
                            let grad = parse_linear_gradient(grandchild, defs);
                            defs.insert(id.to_string(), DefEntry::LinearGradient(grad));
                        }
                    }
                    "radialGradient" => {
                        if let Some(id) = grandchild.attribute("id") {
                            let grad = parse_radial_gradient(grandchild, defs);
                            defs.insert(id.to_string(), DefEntry::RadialGradient(grad));
                        }
                    }
                    "clipPath" => {
                        if let Some(id) = grandchild.attribute("id") {
                            let segments = parse_clip_path_segments(grandchild);
                            defs.insert(id.to_string(), DefEntry::ClipPath(segments));
                        }
                    }
                    _ => {
                        if let Some(id) = grandchild.attribute("id") {
                            defs.insert(id.to_string(), DefEntry::Element(grandchild.clone()));
                        }
                        // grandchild 内にさらに defs がある場合は再帰
                        collect_defs_recursive(grandchild, defs);
                    }
                }
            }
        } else {
            // defs/symbol 以外でも再帰して、ネストされた defs を探す
            collect_defs_recursive(child, defs);
        }
    }
}

/// <clipPath> 内のジオメトリ要素を PathSegment に変換する。
/// 複数の子要素がある場合はすべてのセグメントを結合する。
fn parse_clip_path_segments(clip_el: &xml::XmlElement) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    for child in clip_el.children_elements() {
        if let Some(child_segments) = svg_mod::elements::convert_element(child) {
            segments.extend(child_segments);
        } else if child.tag == "g" {
            // <g> 内のジオメトリ要素も再帰的に処理
            for grandchild in child.children_elements() {
                if let Some(gc_segments) = svg_mod::elements::convert_element(grandchild) {
                    segments.extend(gc_segments);
                }
            }
        }
    }
    segments
}

/// clip-path 属性から url() 参照を解決し、クリップパスセグメントを返す。
/// inline style と presentation attribute の両方をチェックする。
fn resolve_clip_path(el: &xml::XmlElement, defs: &HashMap<String, DefEntry>) -> Option<Vec<PathSegment>> {
    let inline = el.attribute("style")
        .map(svg_mod::style::parse_inline_style)
        .unwrap_or_default();

    let clip_str = inline.get("clip-path")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("clip-path"))?;

    let id = svg_mod::style::extract_url_id(clip_str)?;

    match defs.get(id) {
        Some(DefEntry::ClipPath(segments)) => Some(segments.clone()),
        _ => None,
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
fn build_group(el: &xml::XmlElement, defs: &HashMap<String, DefEntry>, visited: &mut HashSet<String>, ctx: &svg_mod::style::StyleContext) -> SvgGroup {
    let id = el.attribute("id").map(|s| s.to_string());
    let transform = el
        .attribute("transform")
        .map(svg_mod::transform::parse_transform)
        .unwrap_or_else(Affine2D::identity);
    let opacity = svg_mod::style::resolve_opacity(el, ctx);
    let visibility = svg_mod::style::resolve_visibility(el, ctx);
    let clip_path = resolve_clip_path(el, defs);

    // この要素の解決済みスタイルから子要素用の StyleContext を構築
    let child_ctx = build_child_context(el, ctx);

    let mut children = Vec::new();

    for child_el in el.children_elements() {
        process_element(child_el, &mut children, defs, visited, &child_ctx);
    }

    SvgGroup {
        id,
        transform,
        opacity,
        visibility,
        clip_path,
        children,
    }
}

/// 要素のスタイルを解決し、子要素に継承する StyleContext を構築する。
fn build_child_context(el: &xml::XmlElement, parent_ctx: &svg_mod::style::StyleContext) -> svg_mod::style::StyleContext {
    svg_mod::style::StyleContext {
        fill: svg_mod::style::resolve_fill(el, parent_ctx),
        stroke: svg_mod::style::resolve_stroke(el, parent_ctx),
        fill_opacity: parent_ctx.fill_opacity,
        stroke_opacity: parent_ctx.stroke_opacity,
        opacity: parent_ctx.opacity,
        visibility: svg_mod::style::resolve_visibility(el, parent_ctx),
    }
}

/// display 属性が "none" かどうかを判定する。
/// `display` 属性と `style` 属性内のインライン `display: none` の両方をチェックする。
fn is_display_none(el: &xml::XmlElement) -> bool {
    // display 属性を直接チェック
    if el.attribute("display").map(|s| s.eq_ignore_ascii_case("none")).unwrap_or(false) {
        return true;
    }
    // style 属性内の "display: none" をチェック (例: style="display:none; fill:red")
    if let Some(style) = el.attribute("style") {
        for decl in style.split(';') {
            let decl = decl.trim();
            if let Some((prop, val)) = decl.split_once(':') {
                if prop.trim().eq_ignore_ascii_case("display") && val.trim().eq_ignore_ascii_case("none") {
                    return true;
                }
            }
        }
    }
    false
}

/// 単一の XML 要素を処理し、結果を children リストに追加する。
fn process_element(el: &xml::XmlElement, children: &mut Vec<SvgNode>, defs: &HashMap<String, DefEntry>, visited: &mut HashSet<String>, ctx: &svg_mod::style::StyleContext) {
    let tag = el.tag.as_str();

    // SMIL 要素はスキップ (SMIL パーサーで別途処理)
    if SMIL_TAGS.contains(&tag) {
        return;
    }

    // display="none" → 要素とその子孫をシーンツリーから除外
    if is_display_none(el) {
        return;
    }

    // <defs> と <symbol> はシーンツリーに追加しない (定義テーブルで管理)
    if tag == "defs" || tag == "symbol" {
        return;
    }

    // <use> 要素 → 参照先を展開
    if tag == "use" {
        expand_use(el, children, defs, visited, ctx);
        return;
    }

    // <g> 要素 → SvgGroup として再帰処理
    if tag == "g" {
        children.push(SvgNode::Group(build_group(el, defs, visited, ctx)));
        return;
    }

    // ジオメトリ要素 → SvgPath に変換
    if GEOMETRY_TAGS.contains(&tag) {
        if let Some(segments) = svg_mod::elements::convert_element(el) {
            let visibility = svg_mod::style::resolve_visibility(el, ctx);
            let mut fill = svg_mod::style::resolve_fill(el, ctx);
            let mut stroke = svg_mod::style::resolve_stroke(el, ctx);

            // url() 参照の解決 (グラデーション等)
            resolve_paint_url(el, &mut fill, &mut stroke, defs);

            let clip_path = resolve_clip_path(el, defs);

            let path = SvgPath {
                id: el.attribute("id").map(|s| s.to_string()),
                segments,
                fill,
                stroke,
                opacity: svg_mod::style::resolve_opacity(el, ctx),
                visibility,
                clip_path,
            };
            children.push(SvgNode::Path(path));
        }
        return;
    }

    // その他の要素 → 要素自体はスキップするが、子要素にジオメトリが含まれる
    // 可能性があるため再帰的に処理する (例: <clipPath> 内の図形)
    for child_el in el.children_elements() {
        process_element(child_el, children, defs, visited, ctx);
    }
}

/// <use> 要素を展開し、参照先の要素をシーンツリーに追加する。
fn expand_use(
    use_el: &xml::XmlElement,
    children: &mut Vec<SvgNode>,
    defs: &HashMap<String, DefEntry>,
    visited: &mut HashSet<String>,
    ctx: &svg_mod::style::StyleContext,
) {
    // href 属性から参照先 ID を取得 (xlink:href は XML パーサーの ns 除去により href として格納済み)
    let href = match use_el.attribute("href") {
        Some(h) => h,
        None => return,
    };
    let ref_id = href.trim_start_matches('#');

    // 循環参照検出
    if visited.contains(ref_id) {
        return;
    }

    let def_entry = match defs.get(ref_id) {
        Some(entry) => entry,
        None => return,
    };

    let ref_el = match def_entry {
        DefEntry::Element(el) => el,
        // グラデーション定義・クリップパス定義は <use> の展開対象ではない
        DefEntry::LinearGradient(_) | DefEntry::RadialGradient(_) | DefEntry::ClipPath(_) => return,
    };

    visited.insert(ref_id.to_string());

    // x, y 属性を translate 変換として適用
    let x = use_el.attribute("x").and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
    let y = use_el.attribute("y").and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);

    // <use> 自身の transform
    let use_transform = use_el
        .attribute("transform")
        .map(svg_mod::transform::parse_transform)
        .unwrap_or_else(Affine2D::identity);

    // x, y の translate を合成
    let xy_translate = if x != 0.0 || y != 0.0 {
        Affine2D::translate(x, y)
    } else {
        Affine2D::identity()
    };

    let combined_transform = use_transform.multiply(&xy_translate);

    if ref_el.tag == "symbol" {
        // <symbol> の展開: viewBox → width/height のスケーリング
        let symbol_group = expand_symbol(use_el, ref_el, combined_transform, defs, visited, ctx);
        children.push(SvgNode::Group(symbol_group));
    } else if ref_el.tag == "g" {
        // <g> の展開
        let mut group = build_group(ref_el, defs, visited, ctx);
        group.transform = combined_transform.multiply(&group.transform);
        // <use> の opacity も合成
        group.opacity *= svg_mod::style::resolve_opacity(use_el, ctx);
        children.push(SvgNode::Group(group));
    } else if GEOMETRY_TAGS.contains(&ref_el.tag.as_str()) {
        // ジオメトリ要素の展開
        if let Some(segments) = svg_mod::elements::convert_element(ref_el) {
            let visibility = svg_mod::style::resolve_visibility(ref_el, ctx);
            let mut fill = svg_mod::style::resolve_fill(ref_el, ctx);
            let mut stroke = svg_mod::style::resolve_stroke(ref_el, ctx);

            // url() 参照の解決 (グラデーション等)
            resolve_paint_url(ref_el, &mut fill, &mut stroke, defs);

            let path = SvgPath {
                id: use_el.attribute("id").map(|s| s.to_string()),
                segments,
                fill,
                stroke,
                opacity: svg_mod::style::resolve_opacity(ref_el, ctx) * svg_mod::style::resolve_opacity(use_el, ctx),
                visibility,
                clip_path: None,
            };
            // transform を適用するためにグループで包む
            let wrapper = SvgGroup {
                id: None,
                transform: combined_transform,
                opacity: 1.0,
                visibility: true,
                clip_path: None,
                children: vec![SvgNode::Path(path)],
            };
            children.push(SvgNode::Group(wrapper));
        }
    }

    visited.remove(ref_id);
}

/// <symbol> 要素を展開し、viewBox → width/height のスケーリング変換を計算する。
fn expand_symbol(
    use_el: &xml::XmlElement,
    symbol_el: &xml::XmlElement,
    use_transform: Affine2D,
    defs: &HashMap<String, DefEntry>,
    visited: &mut HashSet<String>,
    ctx: &svg_mod::style::StyleContext,
) -> SvgGroup {
    // <use> の width/height (symbol のサイズ)
    let width = use_el.attribute("width").and_then(|s| s.parse::<f32>().ok());
    let height = use_el.attribute("height").and_then(|s| s.parse::<f32>().ok());

    // <symbol> の viewBox
    let view_box = symbol_el.attribute("viewBox").and_then(|s| {
        let nums: Vec<f32> = s
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            Some((nums[0], nums[1], nums[2], nums[3]))
        } else {
            None
        }
    });

    // viewBox → width/height へのスケーリング変換
    let scale_transform = match (view_box, width, height) {
        (Some((vx, vy, vw, vh)), Some(w), Some(h)) => {
            let sx = w / vw;
            let sy = h / vh;
            let scale = sx.min(sy);
            // 中央配置のオフセット
            let dx = (w - vw * scale) / 2.0 - vx * scale;
            let dy = (h - vh * scale) / 2.0 - vy * scale;
            Affine2D {
                a: scale, b: 0.0, c: 0.0, d: scale, e: dx, f: dy,
            }
        }
        _ => Affine2D::identity(),
    };

    let combined = use_transform.multiply(&scale_transform);

    // symbol の子要素を展開
    let mut symbol_children = Vec::new();
    for child_el in symbol_el.children_elements() {
        process_element(child_el, &mut symbol_children, defs, visited, ctx);
    }

    SvgGroup {
        id: use_el.attribute("id").map(|s| s.to_string()),
        transform: combined,
        opacity: svg_mod::style::resolve_opacity(use_el, ctx),
        visibility: svg_mod::style::resolve_visibility(use_el, ctx),
        clip_path: None,
        children: symbol_children,
    }
}

// ============================================
// グラデーションパース
// ============================================

/// <linearGradient> 要素をパースする。
fn parse_linear_gradient(
    el: &xml::XmlElement,
    defs: &HashMap<String, DefEntry>,
) -> LinearGradient {
    // href による継承 (stops を親から継承)
    let parent = el
        .attribute("href")
        .map(|h| h.trim_start_matches('#'))
        .and_then(|id| defs.get(id))
        .and_then(|entry| match entry {
            DefEntry::LinearGradient(g) => Some(g.clone()),
            _ => None,
        });

    let units = match el.attribute("gradientUnits") {
        Some("userSpaceOnUse") => GradientUnits::UserSpaceOnUse,
        _ => GradientUnits::ObjectBoundingBox,
    };

    // objectBoundingBox のデフォルト: x1=0, y1=0, x2=1, y2=0
    // userSpaceOnUse の場合もこれをデフォルトとして使うが、通常は属性で明示される
    let default_x2 = if units == GradientUnits::ObjectBoundingBox {
        1.0
    } else {
        0.0
    };

    let x1 = parse_gradient_coord(el.attribute("x1"))
        .unwrap_or(parent.as_ref().map(|p| p.x1).unwrap_or(0.0));
    let y1 = parse_gradient_coord(el.attribute("y1"))
        .unwrap_or(parent.as_ref().map(|p| p.y1).unwrap_or(0.0));
    let x2 = parse_gradient_coord(el.attribute("x2"))
        .unwrap_or(parent.as_ref().map(|p| p.x2).unwrap_or(default_x2));
    let y2 = parse_gradient_coord(el.attribute("y2"))
        .unwrap_or(parent.as_ref().map(|p| p.y2).unwrap_or(0.0));

    let spread = parse_spread_method(el.attribute("spreadMethod"));

    let transform = el
        .attribute("gradientTransform")
        .map(svg_mod::transform::parse_transform)
        .unwrap_or_else(Affine2D::identity);

    let stops = parse_gradient_stops(el);
    let stops = if stops.is_empty() {
        parent
            .as_ref()
            .map(|p| p.stops.clone())
            .unwrap_or_default()
    } else {
        stops
    };

    LinearGradient {
        x1,
        y1,
        x2,
        y2,
        stops,
        spread,
        transform,
        units,
    }
}

/// <radialGradient> 要素をパースする。
fn parse_radial_gradient(
    el: &xml::XmlElement,
    defs: &HashMap<String, DefEntry>,
) -> RadialGradient {
    let parent = el
        .attribute("href")
        .map(|h| h.trim_start_matches('#'))
        .and_then(|id| defs.get(id))
        .and_then(|entry| match entry {
            DefEntry::RadialGradient(g) => Some(g.clone()),
            _ => None,
        });

    let units = match el.attribute("gradientUnits") {
        Some("userSpaceOnUse") => GradientUnits::UserSpaceOnUse,
        _ => GradientUnits::ObjectBoundingBox,
    };

    let default_c = if units == GradientUnits::ObjectBoundingBox {
        0.5
    } else {
        0.0
    };
    let default_r = if units == GradientUnits::ObjectBoundingBox {
        0.5
    } else {
        0.0
    };

    let cx = parse_gradient_coord(el.attribute("cx"))
        .unwrap_or(parent.as_ref().map(|p| p.cx).unwrap_or(default_c));
    let cy = parse_gradient_coord(el.attribute("cy"))
        .unwrap_or(parent.as_ref().map(|p| p.cy).unwrap_or(default_c));
    let r = parse_gradient_coord(el.attribute("r"))
        .unwrap_or(parent.as_ref().map(|p| p.r).unwrap_or(default_r));
    // fx, fy のデフォルトは cx, cy
    let fx = parse_gradient_coord(el.attribute("fx"))
        .unwrap_or(parent.as_ref().map(|p| p.fx).unwrap_or(cx));
    let fy = parse_gradient_coord(el.attribute("fy"))
        .unwrap_or(parent.as_ref().map(|p| p.fy).unwrap_or(cy));

    let spread = parse_spread_method(el.attribute("spreadMethod"));

    let transform = el
        .attribute("gradientTransform")
        .map(svg_mod::transform::parse_transform)
        .unwrap_or_else(Affine2D::identity);

    let stops = parse_gradient_stops(el);
    let stops = if stops.is_empty() {
        parent
            .as_ref()
            .map(|p| p.stops.clone())
            .unwrap_or_default()
    } else {
        stops
    };

    RadialGradient {
        cx,
        cy,
        r,
        fx,
        fy,
        stops,
        spread,
        transform,
        units,
    }
}

/// グラデーション座標値をパースする。% 表記にも対応。
fn parse_gradient_coord(attr: Option<&str>) -> Option<f32> {
    let s = attr?.trim();
    if let Some(pct_str) = s.strip_suffix('%') {
        let pct: f32 = pct_str.parse().ok()?;
        Some(pct / 100.0)
    } else {
        s.parse::<f32>().ok()
    }
}

/// spreadMethod 属性をパースする。
fn parse_spread_method(attr: Option<&str>) -> SpreadMethod {
    match attr {
        Some(s) if s.eq_ignore_ascii_case("reflect") => SpreadMethod::Reflect,
        Some(s) if s.eq_ignore_ascii_case("repeat") => SpreadMethod::Repeat,
        _ => SpreadMethod::Pad,
    }
}

/// <stop> 子要素をパースしてグラデーションストップのリストを返す。
fn parse_gradient_stops(gradient_el: &xml::XmlElement) -> Vec<GradientStop> {
    let mut stops = Vec::new();
    for child in gradient_el.children_elements() {
        if child.tag == "stop" {
            let inline = child
                .attribute("style")
                .map(svg_mod::style::parse_inline_style)
                .unwrap_or_default();

            // offset
            let offset_str = child.attribute("offset").unwrap_or("0");
            let offset = if let Some(pct_str) = offset_str.strip_suffix('%') {
                pct_str.parse::<f32>().unwrap_or(0.0) / 100.0
            } else {
                offset_str.parse::<f32>().unwrap_or(0.0)
            };
            let offset = offset.clamp(0.0, 1.0);

            // stop-color (inline style > attribute > black)
            let color_str = inline
                .get("stop-color")
                .map(|s| s.as_str())
                .or_else(|| child.attribute("stop-color"))
                .unwrap_or("#000000");
            let color = crate::svg::color::parse_color(color_str)
                .unwrap_or(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                });

            // stop-opacity (inline style > attribute > 1.0)
            let opacity = inline
                .get("stop-opacity")
                .and_then(|s| s.parse::<f32>().ok())
                .or_else(|| {
                    child
                        .attribute("stop-opacity")
                        .and_then(|s| s.parse::<f32>().ok())
                })
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);

            stops.push(GradientStop {
                offset,
                color,
                opacity,
            });
        }
    }
    stops
}

// ============================================
// url() 参照解決
// ============================================

/// fill/stroke の url() 参照を defs テーブルから解決する。
fn resolve_paint_url(
    el: &xml::XmlElement,
    fill: &mut Option<FillStyle>,
    stroke: &mut Option<StrokeStyle>,
    defs: &HashMap<String, DefEntry>,
) {
    let inline = el
        .attribute("style")
        .map(svg_mod::style::parse_inline_style)
        .unwrap_or_default();

    // fill の url() 解決
    let fill_str = inline
        .get("fill")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("fill"));
    if let Some(fill_str) = fill_str {
        if let Some(id) = svg_mod::style::extract_url_id(fill_str) {
            match defs.get(id) {
                Some(DefEntry::LinearGradient(grad)) => {
                    let rule = fill.as_ref().map(|f| f.rule).unwrap_or(FillRule::NonZero);
                    let opacity = fill.as_ref().map(|f| f.opacity).unwrap_or(1.0);
                    *fill = Some(FillStyle {
                        paint: Paint::LinearGradient(grad.clone()),
                        rule,
                        opacity,
                    });
                }
                Some(DefEntry::RadialGradient(grad)) => {
                    let rule = fill.as_ref().map(|f| f.rule).unwrap_or(FillRule::NonZero);
                    let opacity = fill.as_ref().map(|f| f.opacity).unwrap_or(1.0);
                    *fill = Some(FillStyle {
                        paint: Paint::RadialGradient(grad.clone()),
                        rule,
                        opacity,
                    });
                }
                _ => {}
            }
        }
    }

    // stroke の url() 解決
    let stroke_str = inline
        .get("stroke")
        .map(|s| s.as_str())
        .or_else(|| el.attribute("stroke"));
    if let Some(stroke_str) = stroke_str {
        if let Some(id) = svg_mod::style::extract_url_id(stroke_str) {
            match defs.get(id) {
                Some(DefEntry::LinearGradient(grad)) => {
                    if let Some(ref mut s) = stroke {
                        s.paint = Paint::LinearGradient(grad.clone());
                    }
                }
                Some(DefEntry::RadialGradient(grad)) => {
                    if let Some(ref mut s) = stroke {
                        s.paint = Paint::RadialGradient(grad.clone());
                    }
                }
                _ => {}
            }
        }
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
        let defs = HashMap::new();
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
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
        let defs = HashMap::new();
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
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

        let defs = HashMap::new();
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        assert_eq!(group.children.len(), 1);

        match &group.children[0] {
            SvgNode::Path(path) => {
                assert!(!path.segments.is_empty());
                assert!(path.fill.is_some());
                let fill = path.fill.as_ref().unwrap();
                assert_eq!(fill.paint, Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
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

        let defs = HashMap::new();
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
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

        let defs = HashMap::new();
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // rect は含まれるが、set はスキップ
        assert_eq!(group.children.len(), 1);
        assert!(matches!(&group.children[0], SvgNode::Path(_)));
    }

    #[test]
    fn test_build_group_defs_children_excluded() {
        // <defs> 内の要素はシーンツリーに追加されない
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

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // defs 内の rect はシーンツリーに含まれない
        assert_eq!(group.children.len(), 0);
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

    // ----------------------------------------
    // display / visibility テスト
    // ----------------------------------------

    #[test]
    fn test_display_none_excluded() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![
                        ("width".to_string(), "100".to_string()),
                        ("height".to_string(), "50".to_string()),
                        ("display".to_string(), "none".to_string()),
                    ],
                    children: Vec::new(),
                }),
                xml::XmlNode::Element(xml::XmlElement {
                    tag: "circle".to_string(),
                    attrs: vec![
                        ("cx".to_string(), "50".to_string()),
                        ("cy".to_string(), "50".to_string()),
                        ("r".to_string(), "25".to_string()),
                    ],
                    children: Vec::new(),
                }),
            ],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // display="none" の rect は除外され、circle だけ残る
        assert_eq!(group.children.len(), 1);
    }

    #[test]
    fn test_display_none_case_insensitive() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "rect".to_string(),
                attrs: vec![
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                    ("display".to_string(), "None".to_string()),
                ],
                children: Vec::new(),
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // 大文字小文字を区別せず "None" も除外
        assert_eq!(group.children.len(), 0);
    }

    #[test]
    fn test_display_none_in_style_attribute() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "rect".to_string(),
                attrs: vec![
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                    ("style".to_string(), "display: none; fill: red".to_string()),
                ],
                children: Vec::new(),
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // style 属性内の display:none も除外
        assert_eq!(group.children.len(), 0);
    }

    #[test]
    fn test_display_none_group_excludes_children() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "g".to_string(),
                attrs: vec![("display".to_string(), "none".to_string())],
                children: vec![xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![
                        ("width".to_string(), "100".to_string()),
                        ("height".to_string(), "50".to_string()),
                    ],
                    children: Vec::new(),
                })],
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // display="none" の <g> とその子要素はすべて除外
        assert_eq!(group.children.len(), 0);
    }

    #[test]
    fn test_visibility_hidden_kept_in_tree() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "rect".to_string(),
                attrs: vec![
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                    ("visibility".to_string(), "hidden".to_string()),
                ],
                children: Vec::new(),
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // visibility="hidden" でもシーンツリーには含まれる
        assert_eq!(group.children.len(), 1);
        match &group.children[0] {
            SvgNode::Path(p) => assert!(!p.visibility),
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_visibility_default_is_visible() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "rect".to_string(),
                attrs: vec![
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                ],
                children: Vec::new(),
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        assert_eq!(group.children.len(), 1);
        match &group.children[0] {
            SvgNode::Path(p) => assert!(p.visibility),
            _ => panic!("Expected Path"),
        }
        // グループ自体も visibility=true
        assert!(group.visibility);
    }

    #[test]
    fn test_visibility_hidden_on_group() {
        let el = xml::XmlElement {
            tag: "svg".to_string(),
            attrs: vec![],
            children: vec![xml::XmlNode::Element(xml::XmlElement {
                tag: "g".to_string(),
                attrs: vec![("visibility".to_string(), "hidden".to_string())],
                children: vec![xml::XmlNode::Element(xml::XmlElement {
                    tag: "rect".to_string(),
                    attrs: vec![
                        ("width".to_string(), "100".to_string()),
                        ("height".to_string(), "50".to_string()),
                    ],
                    children: Vec::new(),
                })],
            })],
        };

        let defs = collect_defs(&el);
        let group = build_group(&el, &defs, &mut HashSet::new(), &svg_mod::style::StyleContext::default_root());
        // グループはシーンツリーに含まれ、子要素も含まれる
        assert_eq!(group.children.len(), 1);
        match &group.children[0] {
            SvgNode::Group(g) => {
                assert!(!g.visibility);
                // visibility は SVG 仕様に従い継承される。
                // 子要素は親の visibility="hidden" を継承して hidden になる。
                assert_eq!(g.children.len(), 1);
                match &g.children[0] {
                    SvgNode::Path(p) => assert!(!p.visibility),
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    // ----------------------------------------
    // <use> 要素の展開テスト
    // ----------------------------------------

    #[test]
    fn test_use_expands_geometry() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <rect id="r1" width="50" height="50" fill="#FF0000"/>
            </defs>
            <use href="#r1" x="10" y="20"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        // use は Group として展開される (transform を持つため)
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert_eq!(g.children.len(), 1);
                match &g.children[0] {
                    SvgNode::Path(p) => {
                        assert!(p.fill.is_some());
                    }
                    _ => panic!("Expected Path inside use group"),
                }
            }
            _ => panic!("Expected Group for use element"),
        }
    }

    #[test]
    fn test_use_expands_group() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <g id="g1">
                    <rect width="50" height="50"/>
                    <circle cx="25" cy="25" r="10"/>
                </g>
            </defs>
            <use href="#g1"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert_eq!(g.children.len(), 2);
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_use_circular_reference_prevented() {
        // 同一要素を複数回 <use> で参照するケース (循環ではないので両方展開される)
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <rect id="r1" width="50" height="50"/>
            </defs>
            <use href="#r1"/>
            <use href="#r1"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        // 同一要素を複数回使える (循環ではないので)
        assert_eq!(doc.root.children.len(), 2);
    }

    #[test]
    fn test_use_missing_href_ignored() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <use/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 0);
    }

    #[test]
    fn test_use_missing_ref_ignored() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <use href="#nonexistent"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 0);
    }

    // ----------------------------------------
    // StyleContext 継承テスト
    // ----------------------------------------

    #[test]
    fn test_fill_inherited_from_parent_group() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <g fill="#FF0000">
                <rect width="50" height="50"/>
            </g>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                match &g.children[0] {
                    SvgNode::Path(p) => {
                        let fill = p.fill.as_ref().expect("fill should be inherited");
                        assert_eq!(fill.paint, Paint::Color(Color { r: 255, g: 0, b: 0, a: 255 }));
                    }
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_inline_style_overrides_attribute() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <rect width="50" height="50" fill="blue" style="fill: #00FF00"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let fill = p.fill.as_ref().unwrap();
                assert_eq!(fill.paint, Paint::Color(Color { r: 0, g: 255, b: 0, a: 255 }));
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_stroke_inherited_from_parent() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <g stroke="#00FF00" stroke-width="3">
                <rect width="50" height="50"/>
            </g>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                match &g.children[0] {
                    SvgNode::Path(p) => {
                        let stroke = p.stroke.as_ref().expect("stroke should be inherited");
                        assert_eq!(stroke.paint, Paint::Color(Color { r: 0, g: 255, b: 0, a: 255 }));
                        assert_eq!(stroke.width, 3.0);
                    }
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_visibility_inherited_from_parent() {
        // 親 <g> が visibility="hidden" の場合、子要素も hidden を継承する
        let svg = br##"<svg viewBox="0 0 100 100">
            <g visibility="hidden">
                <rect width="50" height="50"/>
            </g>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert!(!g.visibility);
                match &g.children[0] {
                    SvgNode::Path(p) => {
                        // 子要素は親の visibility を継承して hidden
                        assert!(!p.visibility);
                    }
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_fill_none_overrides_inherited() {
        // 親が fill="#FF0000" だが子が fill="none" の場合
        let svg = br##"<svg viewBox="0 0 100 100">
            <g fill="#FF0000">
                <rect width="50" height="50" fill="none"/>
            </g>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                match &g.children[0] {
                    SvgNode::Path(p) => {
                        assert!(p.fill.is_none(), "fill='none' should override inherited fill");
                    }
                    _ => panic!("Expected Path"),
                }
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_inline_style_stroke_overrides_attribute() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <rect width="50" height="50" stroke="red" style="stroke: #0000FF; stroke-width: 5"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let stroke = p.stroke.as_ref().unwrap();
                assert_eq!(stroke.paint, Paint::Color(Color { r: 0, g: 0, b: 255, a: 255 }));
                assert_eq!(stroke.width, 5.0);
            }
            _ => panic!("Expected Path"),
        }
    }

    // ----------------------------------------
    // グラデーションテスト
    // ----------------------------------------

    #[test]
    fn test_linear_gradient_parsed() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <linearGradient id="grad1" x1="0" y1="0" x2="1" y2="0">
                    <stop offset="0" stop-color="#FF0000"/>
                    <stop offset="1" stop-color="#0000FF"/>
                </linearGradient>
            </defs>
            <rect width="100" height="100" fill="url(#grad1)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let fill = p.fill.as_ref().unwrap();
                match &fill.paint {
                    Paint::LinearGradient(grad) => {
                        assert_eq!(grad.stops.len(), 2);
                        assert_eq!(
                            grad.stops[0].color,
                            Color {
                                r: 255,
                                g: 0,
                                b: 0,
                                a: 255
                            }
                        );
                        assert_eq!(
                            grad.stops[1].color,
                            Color {
                                r: 0,
                                g: 0,
                                b: 255,
                                a: 255
                            }
                        );
                    }
                    other => panic!("Expected LinearGradient, got {:?}", other),
                }
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_radial_gradient_parsed() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <radialGradient id="rgrad" cx="0.5" cy="0.5" r="0.5">
                    <stop offset="0" stop-color="white"/>
                    <stop offset="1" stop-color="black"/>
                </radialGradient>
            </defs>
            <circle cx="50" cy="50" r="50" fill="url(#rgrad)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let fill = p.fill.as_ref().unwrap();
                assert!(matches!(&fill.paint, Paint::RadialGradient(_)));
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_gradient_href_inheritance() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <linearGradient id="base">
                    <stop offset="0" stop-color="red"/>
                    <stop offset="1" stop-color="blue"/>
                </linearGradient>
                <linearGradient id="child" href="#base" x1="0" y1="0" x2="0" y2="1"/>
            </defs>
            <rect width="100" height="100" fill="url(#child)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let fill = p.fill.as_ref().unwrap();
                match &fill.paint {
                    Paint::LinearGradient(grad) => {
                        // stops は base から継承
                        assert_eq!(grad.stops.len(), 2);
                        // x2=0, y2=1 は child で上書き
                        assert_eq!(grad.x2, 0.0);
                        assert_eq!(grad.y2, 1.0);
                    }
                    other => panic!("Expected LinearGradient, got {:?}", other),
                }
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_gradient_inline_style() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <linearGradient id="g1">
                    <stop offset="0" stop-color="red"/>
                    <stop offset="1" stop-color="blue"/>
                </linearGradient>
            </defs>
            <rect width="100" height="100" style="fill: url(#g1)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                assert!(matches!(
                    &p.fill.as_ref().unwrap().paint,
                    Paint::LinearGradient(_)
                ));
            }
            _ => panic!("Expected Path"),
        }
    }

    // ----------------------------------------
    // clipPath テスト
    // ----------------------------------------

    #[test]
    fn test_clip_path_parsed() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <clipPath id="clip1">
                    <rect width="50" height="50"/>
                </clipPath>
            </defs>
            <rect width="100" height="100" fill="red" clip-path="url(#clip1)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                assert!(p.clip_path.is_some());
                let clip = p.clip_path.as_ref().unwrap();
                assert!(!clip.is_empty());
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_clip_path_on_group() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <clipPath id="clip1">
                    <circle cx="50" cy="50" r="40"/>
                </clipPath>
            </defs>
            <g clip-path="url(#clip1)">
                <rect width="100" height="100" fill="blue"/>
            </g>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Group(g) => {
                assert!(g.clip_path.is_some());
                assert_eq!(g.children.len(), 1);
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_clip_path_inline_style() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <defs>
                <clipPath id="c1">
                    <rect width="50" height="50"/>
                </clipPath>
            </defs>
            <rect width="100" height="100" style="clip-path: url(#c1)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                assert!(p.clip_path.is_some());
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_no_clip_path() {
        let svg = br##"<svg viewBox="0 0 100 100">
            <rect width="100" height="100"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                assert!(p.clip_path.is_none());
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_clip_path_multiple_children() {
        // clipPath 内に複数のジオメトリ要素がある場合、すべてのセグメントが結合される
        let svg = br##"<svg viewBox="0 0 200 200">
            <defs>
                <clipPath id="clip2">
                    <rect width="100" height="100"/>
                    <circle cx="150" cy="150" r="50"/>
                </clipPath>
            </defs>
            <rect width="200" height="200" fill="green" clip-path="url(#clip2)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                let clip = p.clip_path.as_ref().unwrap();
                // rect (6 segments) + circle (6 segments) = 12 segments
                assert_eq!(clip.len(), 12);
            }
            _ => panic!("Expected Path"),
        }
    }

    #[test]
    fn test_clip_path_undefined_ref() {
        // 存在しない clipPath を参照した場合は None
        let svg = br##"<svg viewBox="0 0 100 100">
            <rect width="100" height="100" clip-path="url(#nonexistent)"/>
        </svg>"##;
        let doc = parse(svg).unwrap();
        match &doc.root.children[0] {
            SvgNode::Path(p) => {
                assert!(p.clip_path.is_none());
            }
            _ => panic!("Expected Path"),
        }
    }
}

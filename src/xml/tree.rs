use super::tokenizer::{self, XmlToken};

#[derive(Debug, Clone, PartialEq)]
pub struct XmlDocument {
    pub root: XmlElement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XmlElement {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn children_elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|n| match n {
            XmlNode::Element(e) => Some(e),
            _ => None,
        })
    }
}

/// UTF-8 バイト列を XML ドキュメントツリーにパースする。
///
/// 処理フロー:
/// 1. tokenizer で XmlToken 列に変換
/// 2. トークン列をスタックベースでツリー構造に変換
/// 3. 名前空間プレフィックスを除去 (タグ名・属性名の両方)
/// 4. xmlns / xmlns:prefix 属性を除去
pub fn parse_xml(data: &[u8]) -> Result<XmlDocument, String> {
    let tokens = tokenizer::tokenize(data)?;

    if tokens.is_empty() {
        return Err("Empty document".to_string());
    }

    // スタックベースのツリー構築
    // stack: 構築中の要素を積む。最終的に stack[0] がルート要素。
    let mut stack: Vec<XmlElement> = Vec::new();

    for token in tokens {
        match token {
            XmlToken::OpenTag { name, attrs } => {
                // 名前空間プレフィックスを除去してから要素を作成し、スタックに積む
                let elem = XmlElement {
                    tag: strip_ns_prefix(&name),
                    attrs: normalize_attrs(attrs),
                    children: Vec::new(),
                };
                stack.push(elem);
            }

            XmlToken::CloseTag { name } => {
                let local_name = strip_ns_prefix(&name);
                // スタックから最上位の要素を取り出し、一つ下の要素の子に追加
                let elem = stack.pop().ok_or_else(|| {
                    format!("Unexpected close tag '</{}>' with empty stack", local_name)
                })?;

                if elem.tag != local_name {
                    return Err(format!(
                        "Mismatched tags: expected '</{}>', found '</{}>'",
                        elem.tag, local_name
                    ));
                }

                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Element(elem));
                } else {
                    // ルート要素の閉じタグ → ドキュメント完成
                    return Ok(XmlDocument { root: elem });
                }
            }

            XmlToken::SelfClosingTag { name, attrs } => {
                let elem = XmlElement {
                    tag: strip_ns_prefix(&name),
                    attrs: normalize_attrs(attrs),
                    children: Vec::new(),
                };

                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Element(elem));
                } else {
                    // ドキュメント全体が自己閉じタグ 1 つだけのケース (稀だが合法)
                    return Ok(XmlDocument { root: elem });
                }
            }

            XmlToken::Text(text) => {
                // 空白のみのテキストは無視する (SVG では意味のある空白テキストはほぼない)
                if text.trim().is_empty() {
                    continue;
                }
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Text(text));
                }
                // ルートレベルのテキストは無視
            }
        }
    }

    // トークンをすべて処理したが CloseTag でドキュメントが完成しなかった場合
    if stack.len() == 1 {
        // 閉じタグがないルート要素 (不正だが救済)
        Ok(XmlDocument {
            root: stack.pop().unwrap(),
        })
    } else if stack.is_empty() {
        Err("No root element found".to_string())
    } else {
        let unclosed: Vec<String> = stack.iter().map(|e| e.tag.clone()).collect();
        Err(format!("Unclosed elements: {:?}", unclosed))
    }
}

/// タグ名・属性名から名前空間プレフィックスを除去する。
/// 例: "svg:rect" → "rect", "xlink:href" → "href"
fn strip_ns_prefix(name: &str) -> String {
    match name.rfind(':') {
        Some(pos) => name[pos + 1..].to_string(),
        None => name.to_string(),
    }
}

/// 属性リストを正規化する:
/// - xmlns / xmlns:* 属性を除去
/// - 属性名の名前空間プレフィックスを除去
fn normalize_attrs(attrs: Vec<(String, String)>) -> Vec<(String, String)> {
    attrs
        .into_iter()
        .filter(|(k, _)| {
            // xmlns 属性自体を除去
            k != "xmlns" && !k.starts_with("xmlns:")
        })
        .map(|(k, v)| {
            // 属性名のプレフィックスを除去
            (strip_ns_prefix(&k), v)
        })
        .collect()
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_document() {
        let input = b"<root></root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "root");
        assert!(doc.root.attrs.is_empty());
        assert!(doc.root.children.is_empty());
    }

    #[test]
    fn test_self_closing_root() {
        let input = b"<root/>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "root");
    }

    #[test]
    fn test_nested_elements() {
        let input = b"<a><b><c/></b></a>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "a");
        assert_eq!(doc.root.children.len(), 1);

        let b = match &doc.root.children[0] {
            XmlNode::Element(e) => e,
            _ => panic!("Expected Element"),
        };
        assert_eq!(b.tag, "b");
        assert_eq!(b.children.len(), 1);

        let c = match &b.children[0] {
            XmlNode::Element(e) => e,
            _ => panic!("Expected Element"),
        };
        assert_eq!(c.tag, "c");
        assert!(c.children.is_empty());
    }

    #[test]
    fn test_text_node() {
        let input = b"<p>Hello</p>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            XmlNode::Text(t) => assert_eq!(t, "Hello"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_whitespace_text_filtered() {
        // 空白のみのテキストノードはフィルタされる
        let input = b"<root>  \n  <child/>  \n  </root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            XmlNode::Element(e) => assert_eq!(e.tag, "child"),
            _ => panic!("Expected Element"),
        }
    }

    #[test]
    fn test_attributes() {
        let input = b"<rect x=\"10\" y=\"20\" width=\"100\" height=\"50\"/>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.attribute("x"), Some("10"));
        assert_eq!(doc.root.attribute("y"), Some("20"));
        assert_eq!(doc.root.attribute("width"), Some("100"));
        assert_eq!(doc.root.attribute("height"), Some("50"));
        assert_eq!(doc.root.attribute("nonexistent"), None);
    }

    #[test]
    fn test_namespace_prefix_stripped_from_tag() {
        // svg:rect → rect
        let input = b"<svg:rect width=\"10\"/>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "rect");
    }

    #[test]
    fn test_namespace_prefix_stripped_from_attribute() {
        // xlink:href → href
        let input = b"<use xlink:href=\"#icon\"/>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.attribute("href"), Some("#icon"));
    }

    #[test]
    fn test_xmlns_attributes_removed() {
        let input = b"<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 400 400\"></svg>";
        let doc = parse_xml(input).unwrap();
        // xmlns, xmlns:xlink は除去されている
        assert_eq!(doc.root.attribute("xmlns"), None);
        // viewBox は残っている
        assert_eq!(doc.root.attribute("viewBox"), Some("0 0 400 400"));
        // 属性は 1 つだけ (viewBox のみ)
        assert_eq!(doc.root.attrs.len(), 1);
    }

    #[test]
    fn test_children_elements_iterator() {
        let input = b"<root><a/><b/>text<c/></root>";
        let doc = parse_xml(input).unwrap();
        let element_tags: Vec<&str> = doc
            .root
            .children_elements()
            .map(|e| e.tag.as_str())
            .collect();
        assert_eq!(element_tags, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_xml_declaration_and_svg() {
        let input = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg viewBox=\"0 0 100 100\"><rect width=\"50\" height=\"50\"/></svg>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "svg");
        assert_eq!(doc.root.attribute("viewBox"), Some("0 0 100 100"));
        assert_eq!(doc.root.children.len(), 1);
        let rect = match &doc.root.children[0] {
            XmlNode::Element(e) => e,
            _ => panic!("Expected Element"),
        };
        assert_eq!(rect.tag, "rect");
        assert_eq!(rect.attribute("width"), Some("50"));
    }

    #[test]
    fn test_mismatched_tags_error() {
        let input = b"<a></b>";
        let result = parse_xml(input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Mismatched tags"));
    }

    #[test]
    fn test_empty_document_error() {
        let result = parse_xml(b"");
        assert!(result.is_err());
    }

    #[test]
    fn test_comment_skipped_in_tree() {
        let input = b"<root><!-- comment --><child/></root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            XmlNode::Element(e) => assert_eq!(e.tag, "child"),
            _ => panic!("Expected Element"),
        }
    }

    #[test]
    fn test_cdata_as_text() {
        let input = b"<root><![CDATA[raw content & <tags>]]></root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 1);
        match &doc.root.children[0] {
            XmlNode::Text(t) => assert_eq!(t, "raw content & <tags>"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_entity_references_in_tree() {
        let input = b"<p>A &amp; B &lt; C</p>";
        let doc = parse_xml(input).unwrap();
        match &doc.root.children[0] {
            XmlNode::Text(t) => assert_eq!(t, "A & B < C"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_multiple_children() {
        let input = b"<root><a x=\"1\"/><b/><c><d/></c></root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 3);

        let tags: Vec<&str> = doc
            .root
            .children_elements()
            .map(|e| e.tag.as_str())
            .collect();
        assert_eq!(tags, vec!["a", "b", "c"]);

        // <a> has attribute x="1"
        let a = doc.root.children_elements().next().unwrap();
        assert_eq!(a.attribute("x"), Some("1"));

        // <c> has child <d>
        let c = doc.root.children_elements().nth(2).unwrap();
        assert_eq!(c.children.len(), 1);
    }

    #[test]
    fn test_realistic_svg_fragment() {
        let input = br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 400 400">
  <g id="group1" transform="translate(10,20)">
    <rect x="0" y="0" width="100" height="50" fill="#FF0000"/>
    <circle cx="50" cy="25" r="10"/>
    <path d="M0 0L10 10Z"/>
  </g>
</svg>"##;
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "svg");
        assert_eq!(doc.root.attribute("viewBox"), Some("0 0 400 400"));
        // xmlns, xmlns:xlink は除去済み
        assert_eq!(doc.root.attribute("xmlns"), None);

        // <g> 要素
        let g = doc.root.children_elements().next().unwrap();
        assert_eq!(g.tag, "g");
        assert_eq!(g.attribute("id"), Some("group1"));
        assert_eq!(g.attribute("transform"), Some("translate(10,20)"));

        // <g> の子要素
        let child_tags: Vec<&str> = g.children_elements().map(|e| e.tag.as_str()).collect();
        assert_eq!(child_tags, vec!["rect", "circle", "path"]);

        // <rect> の属性
        let rect = g.children_elements().next().unwrap();
        assert_eq!(rect.attribute("fill"), Some("#FF0000"));
        assert_eq!(rect.attribute("width"), Some("100"));
    }

    #[test]
    fn test_doctype_skipped() {
        let input = b"<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\"><svg/>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.tag, "svg");
    }

    #[test]
    fn test_mixed_text_and_elements() {
        let input = b"<root>before<child/>after</root>";
        let doc = parse_xml(input).unwrap();
        assert_eq!(doc.root.children.len(), 3);
        match &doc.root.children[0] {
            XmlNode::Text(t) => assert_eq!(t, "before"),
            _ => panic!("Expected Text"),
        }
        match &doc.root.children[1] {
            XmlNode::Element(e) => assert_eq!(e.tag, "child"),
            _ => panic!("Expected Element"),
        }
        match &doc.root.children[2] {
            XmlNode::Text(t) => assert_eq!(t, "after"),
            _ => panic!("Expected Text"),
        }
    }
}

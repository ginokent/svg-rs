/// XML トークン
#[derive(Debug, Clone, PartialEq)]
pub enum XmlToken {
    /// 開始タグ: <name attr1="val1" attr2="val2">
    OpenTag {
        name: String,
        attrs: Vec<(String, String)>,
    },
    /// 閉じタグ: </name>
    CloseTag {
        name: String,
    },
    /// 自己閉じタグ: <name attr1="val1" />
    SelfClosingTag {
        name: String,
        attrs: Vec<(String, String)>,
    },
    /// テキストノード
    Text(String),
}

/// UTF-8 バイト列を XML トークン列にトークナイズする。
///
/// 処理内容:
/// - 開始タグ / 閉じタグ / 自己閉じタグ / テキストを区別
/// - コメント (<!-- -->), PI (<?...?>), DOCTYPE (<!DOCTYPE...>) はスキップ
/// - CDATA セクション (<![CDATA[...]]>) はテキストとして扱う
/// - 属性値内のエンティティ参照 (&amp; &lt; &gt; &quot; &apos;) を解決
pub fn tokenize(data: &[u8]) -> Result<Vec<XmlToken>, String> {
    let input = std::str::from_utf8(data).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < len {
        if bytes[pos] == b'<' {
            // タグまたは特殊構文の開始
            if pos + 1 >= len {
                return Err("Unexpected end of input after '<'".to_string());
            }

            // コメント: <!-- ... -->
            if starts_with_at(bytes, pos, b"<!--") {
                pos = skip_comment(bytes, pos)?;
                continue;
            }

            // CDATA: <![CDATA[ ... ]]>
            if starts_with_at(bytes, pos, b"<![CDATA[") {
                let (text, new_pos) = parse_cdata(bytes, pos)?;
                pos = new_pos;
                if !text.is_empty() {
                    tokens.push(XmlToken::Text(text));
                }
                continue;
            }

            // DOCTYPE: <!DOCTYPE ... >
            // DOCTYPE 内にネストされた '[' ... ']' も処理する
            if starts_with_at_ci(bytes, pos, b"<!DOCTYPE") {
                pos = skip_doctype(bytes, pos)?;
                continue;
            }

            // PI (Processing Instruction): <?...?>
            if starts_with_at(bytes, pos, b"<?") {
                pos = skip_pi(bytes, pos)?;
                continue;
            }

            // 閉じタグ: </name>
            if bytes[pos + 1] == b'/' {
                let (name, new_pos) = parse_close_tag(bytes, pos)?;
                pos = new_pos;
                tokens.push(XmlToken::CloseTag { name });
                continue;
            }

            // 開始タグまたは自己閉じタグ
            let (token, new_pos) = parse_open_tag(bytes, pos)?;
            pos = new_pos;
            tokens.push(token);
        } else {
            // テキストノード: '<' の手前までを読み取る
            let start = pos;
            while pos < len && bytes[pos] != b'<' {
                pos += 1;
            }
            let raw = &input[start..pos];
            let text = resolve_entities(raw);
            // 空白のみのテキストも保持する (ツリー構築側で必要に応じてフィルタする)
            if !text.is_empty() {
                tokens.push(XmlToken::Text(text));
            }
        }
    }

    Ok(tokens)
}

// ============================================
// 内部ヘルパー関数
// ============================================

/// bytes[pos..] が pattern で始まるか判定
fn starts_with_at(bytes: &[u8], pos: usize, pattern: &[u8]) -> bool {
    if pos + pattern.len() > bytes.len() {
        return false;
    }
    &bytes[pos..pos + pattern.len()] == pattern
}

/// bytes[pos..] が pattern で始まるか判定 (ASCII case-insensitive)
fn starts_with_at_ci(bytes: &[u8], pos: usize, pattern: &[u8]) -> bool {
    if pos + pattern.len() > bytes.len() {
        return false;
    }
    bytes[pos..pos + pattern.len()]
        .iter()
        .zip(pattern.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
}

/// コメント <!-- ... --> をスキップし、'-->' の直後の位置を返す
fn skip_comment(bytes: &[u8], pos: usize) -> Result<usize, String> {
    // pos は "<!--" の先頭
    let search_start = pos + 4; // "<!--" の直後から探す
    let len = bytes.len();
    let mut i = search_start;
    while i + 2 < len {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
            return Ok(i + 3);
        }
        i += 1;
    }
    Err(format!("Unterminated comment starting at position {}", pos))
}

/// CDATA <![CDATA[ ... ]]> をパースし、中身のテキストと ']]>' の直後の位置を返す
fn parse_cdata(bytes: &[u8], pos: usize) -> Result<(String, usize), String> {
    // pos は "<![CDATA[" の先頭
    let content_start = pos + 9; // "<![CDATA[" の直後
    let len = bytes.len();
    let mut i = content_start;
    while i + 2 < len {
        if bytes[i] == b']' && bytes[i + 1] == b']' && bytes[i + 2] == b'>' {
            let text = std::str::from_utf8(&bytes[content_start..i])
                .map_err(|e| format!("Invalid UTF-8 in CDATA: {}", e))?;
            return Ok((text.to_string(), i + 3));
        }
        i += 1;
    }
    Err(format!(
        "Unterminated CDATA section starting at position {}",
        pos
    ))
}

/// DOCTYPE <!DOCTYPE ... > をスキップする。
/// DOCTYPE 内の '[' ... ']' (内部サブセット) も正しくスキップする。
fn skip_doctype(bytes: &[u8], pos: usize) -> Result<usize, String> {
    let len = bytes.len();
    let mut i = pos + 9; // "<!DOCTYPE" の直後
    let mut bracket_depth = 0;
    while i < len {
        match bytes[i] {
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b'>' if bracket_depth == 0 => return Ok(i + 1),
            _ => {}
        }
        i += 1;
    }
    Err(format!(
        "Unterminated DOCTYPE starting at position {}",
        pos
    ))
}

/// PI <?...?> をスキップし、'?>' の直後の位置を返す
fn skip_pi(bytes: &[u8], pos: usize) -> Result<usize, String> {
    let len = bytes.len();
    let mut i = pos + 2; // "<?" の直後
    while i + 1 < len {
        if bytes[i] == b'?' && bytes[i + 1] == b'>' {
            return Ok(i + 2);
        }
        i += 1;
    }
    Err(format!("Unterminated PI starting at position {}", pos))
}

/// 閉じタグ </name> をパースし、タグ名と '>' の直後の位置を返す
fn parse_close_tag(bytes: &[u8], pos: usize) -> Result<(String, usize), String> {
    // pos は '<' の位置, bytes[pos+1] == '/'
    let len = bytes.len();
    let name_start = pos + 2; // "</" の直後
    let mut i = name_start;

    // タグ名を読み取る
    while i < len && bytes[i] != b'>' && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let name = std::str::from_utf8(&bytes[name_start..i])
        .map_err(|e| format!("Invalid UTF-8 in close tag name: {}", e))?
        .to_string();

    // '>' を探す (タグ名の後に空白がある場合もある)
    while i < len && bytes[i] != b'>' {
        i += 1;
    }
    if i >= len {
        return Err(format!("Unterminated close tag '</{}'", name));
    }

    Ok((name, i + 1)) // '>' の直後
}

/// 開始タグまたは自己閉じタグをパースする。
/// 返り値: (XmlToken, '>' または '/>' の直後の位置)
fn parse_open_tag(bytes: &[u8], pos: usize) -> Result<(XmlToken, usize), String> {
    let len = bytes.len();
    let name_start = pos + 1; // '<' の直後
    let mut i = name_start;

    // タグ名を読み取る: 空白, '>', '/' のいずれかが出るまで
    while i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' && bytes[i] != b'/' {
        i += 1;
    }
    let name = std::str::from_utf8(&bytes[name_start..i])
        .map_err(|e| format!("Invalid UTF-8 in tag name: {}", e))?
        .to_string();

    if name.is_empty() {
        return Err(format!("Empty tag name at position {}", pos));
    }

    // 属性をパース
    let mut attrs = Vec::new();
    loop {
        // 空白をスキップ
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            return Err(format!("Unterminated tag '<{}'", name));
        }

        // 自己閉じ '/>' の判定
        if bytes[i] == b'/' {
            if i + 1 < len && bytes[i + 1] == b'>' {
                return Ok((
                    XmlToken::SelfClosingTag { name, attrs },
                    i + 2, // '/>' の直後
                ));
            }
            return Err(format!("Expected '>' after '/' in tag '<{}'", name));
        }

        // タグ終端 '>' の判定
        if bytes[i] == b'>' {
            return Ok((
                XmlToken::OpenTag { name, attrs },
                i + 1, // '>' の直後
            ));
        }

        // 属性名=属性値 をパース
        let (attr_name, attr_value, new_i) = parse_attribute(bytes, i)?;
        i = new_i;
        attrs.push((attr_name, attr_value));
    }
}

/// 属性名="属性値" をパースする。
/// 返り値: (属性名, 属性値, 次の読み取り位置)
fn parse_attribute(bytes: &[u8], pos: usize) -> Result<(String, String, usize), String> {
    let len = bytes.len();
    let mut i = pos;

    // 属性名を読み取る: '=' または空白まで
    let attr_name_start = i;
    while i < len && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let attr_name = std::str::from_utf8(&bytes[attr_name_start..i])
        .map_err(|e| format!("Invalid UTF-8 in attribute name: {}", e))?
        .to_string();

    // '=' の前の空白をスキップ
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i >= len || bytes[i] != b'=' {
        return Err(format!("Expected '=' after attribute name '{}'", attr_name));
    }
    i += 1; // '=' をスキップ

    // '=' の後の空白をスキップ
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i >= len {
        return Err(format!(
            "Expected attribute value after '{}='",
            attr_name
        ));
    }

    // 引用符 (' or ") で囲まれた属性値を読み取る
    let quote = bytes[i];
    if quote != b'"' && quote != b'\'' {
        return Err(format!(
            "Expected quote after '{}=', found '{}'",
            attr_name, quote as char
        ));
    }
    i += 1; // 開始引用符をスキップ

    let value_start = i;
    while i < len && bytes[i] != quote {
        i += 1;
    }
    if i >= len {
        return Err(format!(
            "Unterminated attribute value for '{}'",
            attr_name
        ));
    }

    let raw_value = std::str::from_utf8(&bytes[value_start..i])
        .map_err(|e| format!("Invalid UTF-8 in attribute value: {}", e))?;
    let value = resolve_entities(raw_value);

    i += 1; // 終了引用符をスキップ

    Ok((attr_name, value, i))
}

/// XML エンティティ参照を解決する。
/// 対応: &amp; → &, &lt; → <, &gt; → >, &quot; → ", &apos; → '
fn resolve_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'&' {
            // エンティティ参照の開始 → ';' までを読み取って置換
            if let Some(semicolon_pos) = find_byte(bytes, i + 1, b';') {
                let entity = &input[i + 1..semicolon_pos];
                match entity {
                    "amp" => result.push('&'),
                    "lt" => result.push('<'),
                    "gt" => result.push('>'),
                    "quot" => result.push('"'),
                    "apos" => result.push('\''),
                    _ if entity.starts_with('#') => {
                        // 数値文字参照: &#123; または &#x1F;
                        if let Some(ch) = parse_numeric_entity(entity) {
                            result.push(ch);
                        } else {
                            // パースできなければそのまま残す
                            result.push_str(&input[i..=semicolon_pos]);
                        }
                    }
                    _ => {
                        // 未知のエンティティはそのまま残す
                        result.push_str(&input[i..=semicolon_pos]);
                    }
                }
                i = semicolon_pos + 1;
            } else {
                // ';' が見つからない → '&' をそのまま出力
                result.push('&');
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// bytes[start..] 内で target バイトの位置を探す (エンティティ参照の ';' 検索用)
/// 効率のため最大 10 バイトまでしか探索しない (エンティティ参照は短い)
fn find_byte(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    let end = (start + 10).min(bytes.len());
    for i in start..end {
        if bytes[i] == target {
            return Some(i);
        }
    }
    None
}

/// 数値文字参照をパースする: &#123; (10進) or &#x1F; (16進)
fn parse_numeric_entity(entity: &str) -> Option<char> {
    // entity は '#' から始まる (先頭の '&' と末尾の ';' は除去済み)
    let body = &entity[1..]; // '#' をスキップ
    let code_point = if let Some(hex_body) = body.strip_prefix('x').or_else(|| body.strip_prefix('X'))
    {
        u32::from_str_radix(hex_body, 16).ok()?
    } else {
        body.parse::<u32>().ok()?
    };
    char::from_u32(code_point)
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_open_close_tag() {
        let input = b"<root></root>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "root".to_string(),
                    attrs: vec![],
                },
                XmlToken::CloseTag {
                    name: "root".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_self_closing_tag() {
        let input = b"<br/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "br".to_string(),
                attrs: vec![],
            }]
        );
    }

    #[test]
    fn test_self_closing_tag_with_space() {
        let input = b"<br />";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "br".to_string(),
                attrs: vec![],
            }]
        );
    }

    #[test]
    fn test_attributes() {
        let input = b"<div class=\"main\" id='content'></div>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "div".to_string(),
                    attrs: vec![
                        ("class".to_string(), "main".to_string()),
                        ("id".to_string(), "content".to_string()),
                    ],
                },
                XmlToken::CloseTag {
                    name: "div".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_text_node() {
        let input = b"<p>Hello World</p>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "p".to_string(),
                    attrs: vec![],
                },
                XmlToken::Text("Hello World".to_string()),
                XmlToken::CloseTag {
                    name: "p".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_entity_references_in_text() {
        let input = b"<p>A &amp; B &lt; C &gt; D &quot;E&quot; &apos;F&apos;</p>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "p".to_string(),
                    attrs: vec![],
                },
                XmlToken::Text("A & B < C > D \"E\" 'F'".to_string()),
                XmlToken::CloseTag {
                    name: "p".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_entity_references_in_attribute() {
        let input = b"<a href=\"a&amp;b\"></a>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "a".to_string(),
                    attrs: vec![("href".to_string(), "a&b".to_string())],
                },
                XmlToken::CloseTag {
                    name: "a".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_comment_skip() {
        let input = b"<root><!-- this is a comment --><child/></root>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "root".to_string(),
                    attrs: vec![],
                },
                XmlToken::SelfClosingTag {
                    name: "child".to_string(),
                    attrs: vec![],
                },
                XmlToken::CloseTag {
                    name: "root".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_pi_skip() {
        let input = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><root/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "root".to_string(),
                attrs: vec![],
            }]
        );
    }

    #[test]
    fn test_doctype_skip() {
        let input = b"<!DOCTYPE html><root/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "root".to_string(),
                attrs: vec![],
            }]
        );
    }

    #[test]
    fn test_doctype_with_internal_subset() {
        let input = b"<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\" [<!ENTITY foo \"bar\">]><root/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "root".to_string(),
                attrs: vec![],
            }]
        );
    }

    #[test]
    fn test_cdata() {
        let input = b"<root><![CDATA[some <raw> & text]]></root>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "root".to_string(),
                    attrs: vec![],
                },
                XmlToken::Text("some <raw> & text".to_string()),
                XmlToken::CloseTag {
                    name: "root".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_namespace_prefix_in_tag_name() {
        let input = b"<svg:rect width=\"10\"/>";
        let tokens = tokenize(input).unwrap();
        // トークナイザーはプレフィックスを保持する (除去はツリー構築時)
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "svg:rect".to_string(),
                attrs: vec![("width".to_string(), "10".to_string())],
            }]
        );
    }

    #[test]
    fn test_namespace_prefix_in_attribute() {
        let input = b"<use xlink:href=\"#icon\"/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "use".to_string(),
                attrs: vec![("xlink:href".to_string(), "#icon".to_string())],
            }]
        );
    }

    #[test]
    fn test_nested_elements() {
        let input = b"<a><b><c/></b></a>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![
                XmlToken::OpenTag {
                    name: "a".to_string(),
                    attrs: vec![],
                },
                XmlToken::OpenTag {
                    name: "b".to_string(),
                    attrs: vec![],
                },
                XmlToken::SelfClosingTag {
                    name: "c".to_string(),
                    attrs: vec![],
                },
                XmlToken::CloseTag {
                    name: "b".to_string(),
                },
                XmlToken::CloseTag {
                    name: "a".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_multiline_tag() {
        let input = b"<rect\n  x=\"10\"\n  y=\"20\"\n  width=\"100\"\n  height=\"50\"\n/>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "rect".to_string(),
                attrs: vec![
                    ("x".to_string(), "10".to_string()),
                    ("y".to_string(), "20".to_string()),
                    ("width".to_string(), "100".to_string()),
                    ("height".to_string(), "50".to_string()),
                ],
            }]
        );
    }

    #[test]
    fn test_svg_header() {
        let input = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 400 400\"></svg>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens.len(), 3); // whitespace text, OpenTag, CloseTag
        // テキストノード ("\n") は空白のみだが保持される
        assert_eq!(tokens[0], XmlToken::Text("\n".to_string()));
        match &tokens[1] {
            XmlToken::OpenTag { name, attrs } => {
                assert_eq!(name, "svg");
                assert_eq!(attrs.len(), 2);
                assert_eq!(attrs[0], ("xmlns".to_string(), "http://www.w3.org/2000/svg".to_string()));
                assert_eq!(attrs[1], ("viewBox".to_string(), "0 0 400 400".to_string()));
            }
            _ => panic!("Expected OpenTag"),
        }
        assert_eq!(
            tokens[2],
            XmlToken::CloseTag {
                name: "svg".to_string()
            }
        );
    }

    #[test]
    fn test_numeric_entity_decimal() {
        let input = b"<p>&#65;&#66;</p>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[1],
            XmlToken::Text("AB".to_string())
        );
    }

    #[test]
    fn test_numeric_entity_hex() {
        let input = b"<p>&#x41;&#x42;</p>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[1],
            XmlToken::Text("AB".to_string())
        );
    }

    #[test]
    fn test_empty_self_closing() {
        let input = b"<path d=\"M0 0L10 10\" />";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens,
            vec![XmlToken::SelfClosingTag {
                name: "path".to_string(),
                attrs: vec![("d".to_string(), "M0 0L10 10".to_string())],
            }]
        );
    }

    #[test]
    fn test_resolve_entities_no_ampersand() {
        assert_eq!(resolve_entities("hello world"), "hello world");
    }

    #[test]
    fn test_resolve_entities_all_five() {
        assert_eq!(
            resolve_entities("&amp;&lt;&gt;&quot;&apos;"),
            "&<>\"'"
        );
    }

    #[test]
    fn test_resolve_entities_unknown() {
        // 未知のエンティティはそのまま残す
        assert_eq!(resolve_entities("&foo;"), "&foo;");
    }

    #[test]
    fn test_unterminated_comment_error() {
        let input = b"<!-- unterminated";
        assert!(tokenize(input).is_err());
    }

    #[test]
    fn test_unterminated_cdata_error() {
        let input = b"<![CDATA[unterminated";
        assert!(tokenize(input).is_err());
    }

    #[test]
    fn test_unterminated_tag_error() {
        let input = b"<tag attr=\"value\"";
        assert!(tokenize(input).is_err());
    }

    #[test]
    fn test_attribute_with_spaces_around_equals() {
        let input = b"<div class = \"main\"></div>";
        let tokens = tokenize(input).unwrap();
        match &tokens[0] {
            XmlToken::OpenTag { attrs, .. } => {
                assert_eq!(attrs[0], ("class".to_string(), "main".to_string()));
            }
            _ => panic!("Expected OpenTag"),
        }
    }
}

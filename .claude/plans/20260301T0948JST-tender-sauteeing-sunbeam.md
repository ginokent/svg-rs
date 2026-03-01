# XmlDocument の未使用 import に関する調査結果

## Context

前回セッションで `src/xml/mod.rs` の `pub use` から `XmlDocument` を除去した。
ユーザーから「元々 import されていた理由」「本来実装すべき何かが欠けていないか」の確認依頼。

## 調査結果: 問題なし、欠落機能はない

### 元の状態

```rust
// src/xml/mod.rs (修正前)
pub use tree::{XmlDocument, XmlElement, XmlNode, parse_xml};
```

### XmlDocument の定義と使用箇所

| ファイル | 用途 |
|---------|------|
| `src/xml/tree.rs:4` | `pub struct XmlDocument { pub root: XmlElement }` — 型定義 |
| `src/xml/tree.rs:44` | `pub fn parse_xml(...) -> Result<XmlDocument, String>` — 戻り値型 |
| `src/parse.rs:25` | `let mut xml_doc = xml::parse_xml(data)...` — 型推論で使用 |

### なぜ未使用警告が出たか

`parse.rs` は `xml::parse_xml()` を呼ぶが、戻り値の型は **型推論** で解決されるため `XmlDocument` を名前で参照する必要がない:

```rust
// parse.rs — XmlDocument を名前で使っていない
let mut xml_doc = xml::parse_xml(data).map_err(SvgError::XmlParse)?;
//  ^^^^^^^^^ 型推論: xml::tree::XmlDocument
xml_doc.root  // .root フィールドに直接アクセス
```

`smil/parser.rs` も `XmlDocument` ではなく `&XmlElement` を引数に取る設計なので、名前で参照しない。

### なぜ元々 export されていたか

`tree.rs` の全 public 型を一括で re-export する慣習的パターンで含まれていた。
「使うかもしれない」程度の理由で、設計上の必要性はなかった。

### SPEC.ja.md との整合性

SPEC.ja.md (line 281) に明記:

> **目的**: SVG XML を内部ツリー構造にパースする。**公開 API には露出しない。**

`lib.rs` も `mod xml` (非 `pub`) なので、`XmlDocument` がクレート外に公開されることはない。
re-export の有無に関わらず、外部からはアクセス不可。

### 結論

- **欠落機能はない** — `XmlDocument` は内部型であり、型推論で十分に機能している
- **除去は正しい** — 不要な re-export を除去しただけで、動作への影響はゼロ
- **今後必要になった場合** — `crate::xml::tree::XmlDocument` で直接アクセス可能なので、re-export を戻せば済む

## アクション

修正不要。現状のままで正しい。

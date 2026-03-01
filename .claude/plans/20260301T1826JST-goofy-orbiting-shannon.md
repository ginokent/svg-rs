# svg-rs フェーズ 2a: 実装計画

## Context

svg-rs はフェーズ 1 で基本ジオメトリ (rect, circle, ellipse, line, polyline, polygon, path)、単色 fill/stroke、transform、SMIL アニメーションを実装済み。フェーズ 2a では、実用的な SVG (Figma/Illustrator/Inkscape エクスポート、アイコンライブラリ等) を正しくレンダリングするために必要な 7 機能を実装する。`<text>` はフェーズ 2a ではスキップ。

## 実装スコープ (7 ステップ)

1. `<defs>` / `<symbol>` — 定義テーブルの導入 + 既存バグ修正
2. `display` / `visibility` — 表示制御
3. `<use>` — 要素の参照・再利用
4. inline `style` 属性 + 属性の継承 — StyleContext の導入
5. `url()` 参照 + `Paint` enum — グラデーション等の塗り参照基盤
6. `<linearGradient>` / `<radialGradient>` — グラデーション塗り
7. `<clipPath>` — クリッピング

---

## Step 1: `<defs>` / `<symbol>` の正しいハンドリング

**既存バグ修正**: `parse.rs` の `process_element` が `<defs>` を未知要素として扱い、子要素をシーンツリーに追加してしまう (216-218 行目)。SVG 仕様上、`<defs>` 内は参照されるまで描画してはならない。

**変更ファイル:**
- `src/types.rs` — `SvgDocument` に `defs: HashMap<String, DefEntry>` 追加
- `src/parse.rs` — `<defs>` / `<symbol>` の分岐追加。子要素を定義テーブルに格納し、シーンツリーには追加しない

**型設計:**
```rust
/// defs テーブルのエントリ (パース済みデータ)
pub enum DefEntry {
    /// <use> で参照される生の XML 要素 (rect, circle, g 等)
    Element(/* xml::XmlElement のクローン */),
    /// Step 6 で追加: パース済みグラデーション
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    /// Step 7 で追加: パース済みクリップパス
    ClipPath(Vec<PathSegment>),
}
```

**既存テストの修正:**
- `test_build_group_unknown_element_children_processed` (parse.rs 629-653 行目) — `<defs>` 内の `<rect>` がシーンツリーに含まれることを期待しているが、修正後はシーンツリーに含まれない。テストを修正する。

---

## Step 2: `visibility` / `display` 属性

**変更ファイル:**
- `src/types.rs` — `SvgPath`, `SvgGroup` に `display: bool` フィールド追加 (将来の `visibility` 対応を見据える)
- `src/parse.rs` — `display="none"` の要素をシーンツリーに追加しない
- `src/svg/style.rs` — `display`, `visibility` 属性のパース

**実装内容:**
- `display="none"` → 要素自体をシーンツリーから除外 (子要素含む)
- `visibility="hidden"` → 描画スキップ (子要素は独立して visibility を持てる)

---

## Step 3: `<use>` 要素

**変更ファイル:**
- `src/parse.rs` — `<use>` タグの展開ロジック

**実装内容:**
- `href` 属性から参照先 ID を取得 (`#` 除去)。`xlink:href` は XML パーサーの名前空間除去 (`xml/tree.rs` 133-138 行目) により自動的に `href` として格納済み
- 定義テーブル (`DefEntry::Element`) から参照先を取得
- `x`, `y` 属性を translate として適用
- `<symbol>` 参照時: `viewBox` → `width`/`height` のスケーリング変換を計算
- 循環参照検出 (visited set で無限ループ防止)
- 参照先要素を再帰的にシーンツリーに展開

---

## Step 4: inline `style` 属性 + 属性の継承 (統合)

inline style と属性継承は密結合 (CSS specificity: inline style > presentation attributes > 継承値 > デフォルト) のため、同一ステップで実装する。

**変更ファイル:**
- `src/svg/style.rs` — `parse_inline_style()` 関数追加 + 継承対応
- `src/parse.rs` — `StyleContext` 構造体を導入し、`build_group` / `process_element` で親から子へ引き回す

**StyleContext の設計:**
```rust
/// 親から継承されるスタイル属性群
struct StyleContext {
    fill: Option<FillStyle>,       // 継承可能
    stroke: Option<StrokeStyle>,   // 継承可能
    fill_opacity: f32,
    stroke_opacity: f32,
    opacity: f32,
    visibility: bool,
    // ... 他の継承可能な属性
}
```

**スタイル解決の優先度:**
1. `style` 属性 (inline) — 最高優先
2. presentation attributes (`fill="red"` 等)
3. 継承された値 (親の `StyleContext`)
4. SVG デフォルト値

**inline style のパース:**
- `style="fill:#f00; stroke:none; opacity:0.5"` → セミコロン区切りで key:value に分割
- 対応プロパティ: `fill`, `fill-rule`, `fill-opacity`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `stroke-dasharray`, `stroke-dashoffset`, `stroke-opacity`, `opacity`, `display`, `visibility`
- この時点では `fill:url(#grad)` は未対応 (Step 5 で対応)

---

## Step 5: `url()` 参照 + `Paint` enum

**注意: `Paint` 名前衝突**
`render.rs:13` で `use tiny_skia::{Paint, ...}` が既にインポートされている。svg-rs の `Paint` enum と衝突するため、`render.rs` 内では qualified path (`svg::Paint`) を使用するか、`use svg::Paint as SvgPaint` でリネームする。

**変更ファイル:**
- `src/types.rs` — `Paint` enum 追加。`FillStyle.color: Color` → `FillStyle.paint: Paint`、`StrokeStyle.color: Color` → `StrokeStyle.paint: Paint` に変更
- `src/svg/style.rs` — `url(#id)` パターンの検出
- `src/parse.rs` — パース後に url() 参照を定義テーブルで解決 (2nd pass)。`defs` テーブルは公開 API に含めない (`parse()` 内で解決を完結)
- `src/bin/cli/render.rs` — `Paint` に応じた描画分岐 (tiny_skia の `Paint` との名前衝突に注意)
- `src/bin/cli/animation.rs` — `resolve_fill` / `resolve_stroke` の型変更追従 (92-132 行目)
- `src/path/stroke.rs` — テスト内の `StrokeStyle` 構築の修正 (493 行目)
- `tests/checkout.rs` — 型変更が必要な箇所があれば修正

**注意: `smil/evaluator.rs` は変更不要** — evaluator は `SmilValue` のみ扱い、`FillStyle` / `StrokeStyle` に触れない。変更は `animation.rs` のみ。

**公開型としての `Paint` enum:**
```rust
pub enum Paint {
    Color(Color),
    LinearGradient(LinearGradient),  // Step 6 で値を入れる
    RadialGradient(RadialGradient),  // Step 6 で値を入れる
    None,
}
```

**url() 参照の解決タイミング:**
- パース時に `parse()` 関数内でシーンツリー構築後、2nd pass で `url(#id)` を定義テーブルから解決
- `SvgDocument` は完全に解決済みの状態で返す (利用者が `Ref` を気にしなくてよい)
- 内部的には一時的に参照 ID を `Option<String>` で保持し、解決後に `Paint` に変換
- `defs` テーブルは `parse.rs` 内部のみで使用 (公開 API に含めない。`xml::XmlElement` が非公開モジュールのため)

**Breaking change の影響範囲 (全ファイル):**
- `src/types.rs` — 型定義変更 (`FillStyle.color` と `StrokeStyle.color` 両方)
- `src/svg/style.rs` — パース関数の戻り値変更 + テスト修正 (141, 156 行目)
- `src/parse.rs` — FillStyle 構築箇所 + テスト修正 (543 行目)
- `src/path/stroke.rs` — テスト内の StrokeStyle 構築 (493 行目)
- `src/bin/cli/render.rs` — `paint` フィールドの match 分岐 (89, 92, 106, 109 行目) + Paint 名前衝突の解消
- `src/bin/cli/animation.rs` — `result.color = *c` → `result.paint = Paint::Color(*c)` (104, 126 行目)

---

## Step 6: `<linearGradient>` / `<radialGradient>`

**変更ファイル:**
- `src/types.rs` — `LinearGradient`, `RadialGradient`, `GradientStop`, `SpreadMethod`, `GradientUnits` 追加
- `src/parse.rs` — `<defs>` 内のグラデーション要素パース。`<stop>` 子要素の解析。`gradientTransform` は既存の `svg::transform::parse_transform()` を再利用
- `src/bin/cli/render.rs` — tiny-skia の Shader でグラデーション描画

**型定義:**
```rust
pub struct LinearGradient {
    pub x1: f32, pub y1: f32,
    pub x2: f32, pub y2: f32,
    pub stops: Vec<GradientStop>,
    pub spread: SpreadMethod,       // Pad | Reflect | Repeat
    pub transform: Affine2D,        // gradientTransform
    pub units: GradientUnits,       // UserSpaceOnUse | ObjectBoundingBox
}
pub struct RadialGradient {
    pub cx: f32, pub cy: f32, pub r: f32,
    pub fx: f32, pub fy: f32,
    pub stops: Vec<GradientStop>,
    pub spread: SpreadMethod,
    pub transform: Affine2D,
    pub units: GradientUnits,
}
pub struct GradientStop {
    pub offset: f32,  // 0.0-1.0
    pub color: Color,
    pub opacity: f32,
}
```

**追加対応:**
- `href` によるグラデーション継承 (stops を親から継承)
- `gradientUnits`: `objectBoundingBox` (デフォルト) / `userSpaceOnUse`

---

## Step 7: `<clipPath>`

**変更ファイル:**
- `src/types.rs` — `SvgPath`, `SvgGroup` に `clip_path_id: Option<String>` 追加。`ClipPath` 構造体追加
- `src/parse.rs` — `<defs>` 内の `<clipPath>` 要素パース (子要素を PathSegment に変換して `DefEntry::ClipPath` として格納)
- `src/svg/style.rs` — `clip-path="url(#clip0)"` 属性のパース
- `src/bin/cli/render.rs` — tiny-skia の `ClipMask` 適用

**実装内容:**
- `<clipPath>` 内のジオメトリ要素を PathSegment に変換 (既存の `svg::elements` を再利用)
- `clipPathUnits`: `userSpaceOnUse` (デフォルト) / `objectBoundingBox`
- レンダリング時: PathSegment → flatten → tiny-skia `ClipMask` に変換

---

## 変更対象ファイル一覧

| ファイル | Step | 変更内容 |
|---------|------|---------|
| `src/types.rs` | 1,2,5,6,7 | DefEntry, display, Paint enum, グラデーション型, clip_path_id |
| `src/parse.rs` | 1,2,3,4,5,6,7 | defs 処理, display, use 展開, StyleContext, url() 解決, グラデーションパース, clipPath パース |
| `src/svg/style.rs` | 2,4,5,7 | display/visibility, inline style, url() 検出, clip-path 属性 |
| `src/bin/cli/render.rs` | 5,6,7 | Paint 対応描画, グラデーション描画, ClipMask |
| `src/bin/cli/animation.rs` | 5 | FillStyle/StrokeStyle の型変更追従 (resolve_fill, resolve_stroke) |
| `src/path/stroke.rs` | 5 | テスト内の StrokeStyle 構築修正 |
| `tests/checkout.rs` | 5 | 型変更があれば修正 |

## 再利用する既存コード

- `src/svg/color.rs` — グラデーション stop の色パース
- `src/svg/transform.rs` — `gradientTransform` パース
- `src/xml/tree.rs` — `XmlElement` 型 (defs テーブルの値)
- `src/svg/elements.rs` — clipPath 内の図形 → PathSegment 変換
- `src/path/flatten.rs` — clipPath の ClipMask 生成時のフラッテニング

## 検証方法

1. **ユニットテスト**: 各 Step の実装ごとにテスト追加
2. **統合テスト**: `tests/` に Phase 2a 用テスト SVG ファイルを追加
3. **CLI 目視確認**: 以下のパターンの SVG を PNG 出力して検証
   - `<defs>` + `<use>` パターン (アイコン sprite)
   - `<linearGradient>` / `<radialGradient>` を使った図形
   - `<clipPath>` でクリップされた図形
   - `style` 属性でスタイリングされた図形 (Illustrator 風)
   - `<g fill="red">` の子要素への継承
   - `display="none"` で非表示の要素
4. **`cargo test`**: 全テスト通過
5. **`cargo clippy`**: 警告なし

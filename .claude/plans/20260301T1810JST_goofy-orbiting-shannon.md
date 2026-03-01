# svg-rs フェーズ 2: 未実装機能の調査結果と優先度

## Context

svg-rs はフェーズ 1 で基本的なジオメトリ要素 (rect, circle, ellipse, line, polyline, polygon, path)、単色 fill/stroke、transform、SMIL アニメーションを実装済み。しかし、実用的な SVG (Figma/Illustrator/Inkscape エクスポート、アイコンライブラリ等) を正しくレンダリングするには多くの機能が不足している。

---

## 現在の実装における構造的課題

### 1. `<defs>` 内の要素が直接レンダリングされるバグ
`parse.rs` が `<defs>` を未知要素として扱い、子要素をシーンツリーに追加してしまう。SVG 仕様上、`<defs>` 内は参照されるまで描画してはならない。

### 2. `FillStyle` が単色限定
`FillStyle.color: Color` のみで `url(#gradient)` 等の参照を表現できない。`Paint` enum への拡張が必要。

### 3. ID 参照メカニズムの不在
`<use href="#id">` や `fill="url(#grad)"` の解決に必要な ID → 要素のルックアップテーブルが存在しない。

---

## 優先度付き機能一覧

### P0: 必須 (他機能の前提となる基盤)

| # | 機能 | 出現頻度 | 難易度 | 依存 | 概要 |
|---|------|---------|--------|------|------|
| P0-1 | `<defs>` / `<symbol>` | 高 | 低 | なし | 定義テーブル (`HashMap<String, DefNode>`) の導入。`<defs>` 内を描画しない |
| P0-2 | `<use>` | 高 | 中 | P0-1 | `href="#id"` で定義を参照・展開。アイコンライブラリの基本パターン |
| P0-3 | `url()` 参照 + `Paint` enum | 高 | 中 | P0-1 | `fill="url(#grad)"` のパース。`FillStyle.color` → `Paint` enum への拡張 |
| P0-4 | `visibility` / `display` | 高 | 低 | なし | `display="none"` で描画スキップ、`visibility="hidden"` 対応 |

### P1: 重要 (実用的な SVG の多くで使用)

| # | 機能 | 出現頻度 | 難易度 | 依存 | 概要 |
|---|------|---------|--------|------|------|
| P1-1 | `<linearGradient>` / `<radialGradient>` | 高 | 中〜高 | P0-1, P0-3 | グラデーション塗り。`<stop>` 要素、`gradientTransform`、`spreadMethod` 対応 |
| P1-2 | `<clipPath>` | 高 | 中 | P0-1, P0-3 | クリッピング。Figma のフレーム (overflow hidden) で必須 |
| P1-3 | CSS `style` 属性 (インライン) | 高 | 低〜中 | なし | `style="fill:#000;stroke:none"` のパース。Illustrator エクスポートで必須 |
| P1-4 | 属性の継承 | 中〜高 | 中 | なし | `<g fill="red">` の子要素への暗黙的継承。SVG スタイリングの基本 |

### P2: あると良い (互換性向上)

| # | 機能 | 出現頻度 | 難易度 | 依存 | 概要 |
|---|------|---------|--------|------|------|
| P2-1 | `<style>` タグ (CSS ルール) | 中 | 高 | P1-3 | `.cls-1 { fill: #333; }` 等。Inkscape エクスポートで使用 |
| P2-2 | `class` 属性 | 中 | 低 | P2-1 | CSS ルールのマッチングに使用 |
| P2-3 | `<mask>` | 中 | 中〜高 | P0-1, P0-3 | 透過マスク。Figma のマスク機能 |
| P2-4 | `<marker>` | 中 | 中 | P0-1, P0-3 | 矢印等。フローチャート、UML 図で使用 |
| P2-5 | `<image>` | 中 | 中〜高 | なし | data URI / 外部画像の埋め込み |
| P2-6 | `animateMotion` | 低〜中 | 中 | なし | パスに沿ったアニメーション |

### P3: 将来検討 (実装コストが非常に高い)

| # | 機能 | 出現頻度 | 難易度 | 依存 | 概要 |
|---|------|---------|--------|------|------|
| P3-1 | `<filter>` | 中 | 高 | P0-1, P0-3 | feGaussianBlur, feColorMatrix 等。ピクセル処理パイプライン |
| P3-2 | `<pattern>` | 低 | 高 | P0-1, P0-3 | パターン塗り |
| P3-3 | `<text>` / `<tspan>` | 高 (※) | 非常に高 | フォント処理 | ※ 多くのツールはテキストをパスに変換してエクスポートするため実質的影響は限定的。外部依存ゼロの制約と矛盾する可能性あり |

---

## デザインツール別の必要機能

| 機能 | Figma | Illustrator | Inkscape | アイコンライブラリ | Web SVG |
|------|-------|-------------|----------|-----------------|---------|
| defs/use | ★★★ | ★★ | ★★★ | ★★★ | ★★★ |
| グラデーション | ★★★ | ★★★ | ★★ | ★ | ★★★ |
| clipPath | ★★★ | ★★ | ★★ | ★ | ★★ |
| style 属性 | ★ | ★★★ | ★ | ★ | ★★ |
| `<style>` タグ | ★ | ★★ | ★★★ | ★ | ★★ |
| 属性継承 | ★★ | ★★ | ★★ | ★★ | ★★★ |
| visibility | ★★ | ★★ | ★ | ★ | ★★ |

---

## 推奨実装順序

```
Phase 2a (基盤):
  1. P0-4: visibility/display        ← 最も簡単、即座に効果
  2. P1-3: CSS style 属性 (inline)   ← Illustrator SVG 対応
  3. P1-4: 属性継承                   ← SVG スタイリングの基本
  4. P0-1: defs/symbol               ← 後続機能の全前提
  5. P0-2: use                       ← アイコンライブラリ対応
  6. P0-3: url() + Paint enum        ← グラデーションの前提
  7. P1-1: グラデーション              ← 見た目を大幅改善
  8. P1-2: clipPath                  ← Figma SVG 対応

Phase 2b (拡張):
  9.  P2-1 + P2-2: <style> タグ + class
  10. P2-3: mask
  11. P2-4: marker
  12. P2-5: image
  13. P2-6: animateMotion

Phase 3 (将来):
  14. P3-1: filter
  15. P3-2: pattern
  16. P3-3: text
```

## 主な変更対象ファイル

- `src/types.rs` — `Paint` enum, グラデーション型, `defs` テーブル, `visibility` フィールド等
- `src/parse.rs` — defs 処理, use 展開, style 継承コンテキスト, display/visibility
- `src/svg/style.rs` — `url()` 参照, CSS style 属性, 属性継承
- `src/bin/cli/render.rs` — `Paint` 対応描画, clipPath の ClipMask 適用
- `src/bin/cli/animation.rs` — `FillStyle`/`StrokeStyle` の型変更追従

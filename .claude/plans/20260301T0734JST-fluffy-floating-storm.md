# SVG パーサー & SMIL エバリュエーター + svg-cli 実装プラン

## Context

SPEC.ja.md に基づき、**外部依存ゼロの SVG パースライブラリ** (`svg` クレート) と、
その利用例としての **CLI レンダラー** (`svg-cli` バイナリクレート) を実装する。

`svg` クレートは SVG → `SvgDocument` パース、パスフラッテニング、ストローク→フィル変換、
SMIL アニメーション評価を提供する汎用ライブラリ。レンダリングバックエンドには依存しない。

`svg-cli` は `svg` + `tiny-skia` + `png` + `clap` を組み合わせて
SVG → PNG / APNG 変換を行うバイナリ。

## クレート構成

| クレート | 依存 | 用途 |
|---------|------|------|
| `svg` (lib) | `std` のみ | SVG パース + SMIL 評価 + パス処理 |
| `svg-cli` (bin) | `svg`, `tiny-skia`, `png`, `clap` | CLI レンダラー |

## `svg` ライブラリ - モジュール構成 (SPEC.ja.md 準拠)

```
src/
  lib.rs              # parse(), flatten(), stroke_to_fill(), evaluate()
  types.rs            # 全公開データ型
  parse.rs            # parse() のエントリポイント
  xml/
    mod.rs            # XML パーサーモジュール
    tokenizer.rs      # バイト列 → トークン
    tree.rs           # トークン → XmlDocument ツリー
  svg/
    mod.rs
    elements.rs       # SVG 要素 → PathSegment
    path_data.rs      # `d` 属性パーサー
    transform.rs      # transform → Affine2D
    color.rs          # 色パース (#hex, rgb, rgba, named)
    style.rs          # fill/stroke 属性パース
  path/
    mod.rs
    flatten.rs        # cubic bezier → 折れ線
    stroke.rs         # ストローク → フィルパス
  smil/
    mod.rs
    parser.rs         # SMIL 要素パース
    evaluator.rs      # アニメーション値計算
```

## 実装ステップ (SPEC.ja.md §実装の優先順位 に準拠)

### Step 1: プロジェクト基盤
- `Cargo.toml`: edition="2021", 依存なし, `[[bin]]` 追加
- `src/types.rs`: SPEC 記載の全公開型を定義
- `src/error.rs` (types.rs 内): `SvgError` + `Display` + `Error` impl
- `src/lib.rs`: モジュール宣言 + `pub use types::*` + API シグネチャ (stub)
- 全モジュールのスケルトン作成
- `cargo check` 通過確認

### Step 2: XML パーサー (`xml/`)
- `tokenizer.rs`: UTF-8 バイト列 → XML トークン (開始タグ、終了タグ、自己閉じ、テキスト、コメント)
- `tree.rs`: トークン列 → `XmlDocument` / `XmlElement` ツリー
- 対応: 名前空間プレフィックス除去、エンティティ参照 (`&amp;` 等)、コメント/DOCTYPE/PI スキップ
- ユニットテスト: 基本 XML、名前空間、自己閉じ、エンティティ

### Step 3: SVG path `d` パーサー (`svg/path_data.rs`)
- M/m, L/l, H/h, V/v, C/c, S/s, Q/q, T/t, A/a, Z/z の全コマンド対応
- 全て `MoveTo`, `LineTo`, `CubicTo`, `Close` に正規化
- 二次→三次ベジェ昇格、楕円弧→三次ベジェ近似
- 相対→絶対変換、暗黙繰り返し
- ユニットテスト: fixture 内の各パスデータ + 全コマンド型

### Step 4: transform パーサー (`svg/transform.rs`)
- `Affine2D` の全メソッド実装 (identity, translate, rotate, scale, skew, multiply, transform_point)
- `parse_transform()`: translate, scale, rotate, skewX, skewY, matrix
- 複数 transform 連結
- ユニットテスト: 各関数の行列値、連結

### Step 5: 色パーサー (`svg/color.rs`)
- `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`, `none`
- CSS Named Colors 148 色テーブル
- ユニットテスト: fixture 内の全色

### Step 6: fill/stroke パーサー (`svg/style.rs`)
- `parse_fill()`, `parse_stroke()`, `parse_opacity()`
- デフォルト値: fill=black, stroke=none, opacity=1.0
- stroke-dasharray, stroke-dashoffset パース
- ユニットテスト

### Step 7: SVG 要素 → パス変換 (`svg/elements.rs`)
- rect (角丸対応), circle, ellipse, line, polyline, polygon → `Vec<PathSegment>`
- 円/楕円: cubic Bezier 近似 (k = 0.5522847498)
- 角丸 rect: 4 辺 + 4 角の CubicTo
- ユニットテスト: 各要素の変換結果

### Step 8: 統合パーサー (`parse.rs`)
- `pub fn parse(data: &[u8]) -> Result<SvgDocument, SvgError>`
- XML パース → SVG ツリー走査 → `SvgDocument` 構築
- `<svg>` viewBox 取得
- 再帰的に `<g>`, 各ジオメトリ要素を処理
- id なしアニメーションターゲットに自動 ID 付与
- 統合テスト: `checkout.svg` パース → ツリー構造・アニメーション数検証

### Step 9: パスフラッテニング (`path/flatten.rs`)
- 適応的 de Casteljau 分割
- `pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<(f32, f32)>>`
- サブパスごとに分割して返す
- ユニットテスト: 直線 (変化なし)、曲線 (分割)、tolerance 精度

### Step 10: SMIL パーサー (`smil/parser.rs`)
- `<animate>`, `<set>`, `<animateTransform>` の属性パース
- values, keyTimes, keySplines, dur, repeatCount, calcMode, additive, fill, begin
- ターゲット ID 解決 (子要素 / href)
- SmilValue パース (attributeName に応じた型判定)
- ユニットテスト: fixture 内の全アニメーション要素

### Step 11: SMIL エバリュエーター (`smil/evaluator.rs`)
- `pub fn evaluate(anim: &SmilAnimation, elapsed_secs: f64) -> Option<SmilValue>`
- 時刻正規化、区間検索、keySplines Bezier 補間
- calcMode: Linear, Discrete, Spline, Paced
- repeatCount, begin, fill (freeze/remove) 処理
- ユニットテスト: 各 calcMode、freeze/remove、repeat

### Step 12: ストローク → フィル変換 (`path/stroke.rs`)
- `pub fn stroke_to_fill(segments: &[PathSegment], style: &StrokeStyle) -> Vec<PathSegment>`
- 法線オフセット → 外形/内形パス生成
- LineCap (Butt, Round, Square), LineJoin (Miter, Round, Bevel)
- DashPattern 適用
- 段階的: まず LineTo のみ → CubicTo → Dash

### Step 13: svg-cli バイナリ
- Cargo.toml に `[[bin]]` + 依存追加 (tiny-skia, png, clap)
- CLI: `-i input.svg -o output.{png,apng} [--fps N] [--duration Ns] [--width W] [--height H]`
- パイプライン:
  1. `svg::parse()` で SVG パース
  2. 出力形式判定 (.png → 静的, .apng → アニメーション)
  3. 各フレーム: SMIL 評価 → アニメーション属性適用 → フラッテニング → tiny-skia 描画
  4. PNG/APNG エンコード出力
- ストリーミング出力 (1 フレームずつ描画→書き込み→解放)

### Step 14: 統合テスト & 検証
- `checkout.svg` → PNG 出力 (t=0) で目視確認
- `checkout.svg` → APNG 出力 (30fps, 3s) でアニメーション確認
- ユニットテスト全通過

## 検証方法

```bash
# ライブラリテスト
cargo test

# 静的 PNG 出力
cargo run --bin svg-cli -- -i tests/fixtures/checkout.svg -o /tmp/checkout.png

# APNG 出力
cargo run --bin svg-cli -- -i tests/fixtures/checkout.svg -o /tmp/checkout.apng --fps 30 --duration 3s
```

## 重要な設計判断

1. **外部依存ゼロ**: `svg` クレートは `std` のみ。XML パーサーも自前実装
2. **全要素を PathSegment に正規化**: rect/circle/ellipse 等も全て PathSegment 列に変換し、統一的に扱う
3. **CubicTo への正規化**: 二次ベジェ、楕円弧も全て三次ベジェに変換
4. **SMIL アニメーションはドキュメントレベルで管理**: `SvgDocument.animations` に全アニメーションを集約、target_id で要素と紐付け
5. **レンダリングは svg-cli の責務**: ライブラリはデータ構造提供のみ

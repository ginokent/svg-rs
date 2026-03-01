# SVG ライブラリ完成 + svg-cli 実装プラン

## Context

前回セッションで `svg` クレートの全モジュール (Step 1-12) が実装済み (約 8,000 行)。
残りは SMIL 統合の 1 箇所の TODO 修正、コンパイル確認、統合テスト追加、
および svg-cli バイナリ (tiny-skia + png + clap) の実装。

## Step A: SMIL 統合 + コンパイル警告修正

### A-1: parse.rs の SMIL スタブを実装に置換

`src/parse.rs` 47-49 行目:
```rust
// 現在 (スタブ):
// TODO: Step 10 で SMIL パーサーが実装されたら以下に差し替える:
//   let animations = crate::smil::parser::parse_smil_animations(&xml_doc.root);
let animations = Vec::new();

// 修正後:
let animations = crate::smil::parser::parse_smil_animations(&xml_doc.root);
```

### A-2: コンパイル + テスト確認

```bash
cargo check 2>&1  # 警告確認
cargo test         # 全テスト通過確認
```

## Step B: 統合テスト追加

### B-1: parse.rs に SMIL 統合テスト追加

```rust
#[test]
fn test_parse_svg_with_animations() {
    // animate 要素を含む SVG をパースし、animations に正しく取得されることを確認
}
```

### B-2: tests/checkout.rs 統合テスト作成

- `checkout.svg` のパース結果検証 (viewBox, パス数, アニメーション数)
- SMIL 評価テスト (t=0, t=1.0 での値確認)
- 全パスのフラッテニング可能性テスト

対象ファイル: `tests/checkout.rs` (新規)

## Step C: svg-cli バイナリ実装

### C-1: Cargo.toml に cli feature gate 追加

```toml
[[bin]]
name = "svg-cli"
path = "src/bin/cli/main.rs"
required-features = ["cli"]

[features]
default = []
cli = ["dep:tiny-skia", "dep:png", "dep:clap"]

[dependencies.tiny-skia]
version = "0.11"
optional = true

[dependencies.png]
version = "0.17"
optional = true

[dependencies.clap]
version = "4"
features = ["derive"]
optional = true
```

**設計判断**: optional dependencies + feature gate でライブラリの依存ゼロを維持。

### C-2: CLI エントリポイント (`src/bin/cli/main.rs`)

- clap による引数パース: `-i input.svg -o output.{png,apng} [--fps N] [--duration N] [--width W] [--height H]`
- 出力拡張子で静止画/アニメーション判定

### C-3: レンダリングエンジン (`src/bin/cli/render.rs`)

レンダリングパイプライン:
1. `svg::parse()` で SVG パース
2. viewBox → 出力サイズへのスケーリング
3. シーンツリーの再帰描画:
   - `SvgGroup`: transform 合成 + 子要素描画
   - `SvgPath`: `PathSegment` → `tiny_skia::PathBuilder` に直接マッピング
   - Fill: `pixmap.fill_path()` で描画
   - Stroke: `pixmap.stroke_path()` で描画 (tiny-skia のネイティブストローク)

**重要**: `svg::stroke_to_fill()` は CLI では不使用。tiny-skia が stroke_path を直接サポートするため。

### C-4: SMIL アニメーション適用 (`src/bin/cli/animation.rs`)

- target_id でアニメーションをフィルタ
- `svg::evaluate(anim, elapsed)` で現在値を取得
- attribute に応じて transform / opacity / fill / stroke / stroke-dashoffset に適用
- additive=sum の場合は合成

### C-5: APNG エンコード (`src/bin/cli/apng.rs`)

- `png` クレートの `set_animated()` API で APNG 出力
- フレームループ: elapsed = frame_idx / fps
- tiny-skia の premultiplied alpha → straight alpha への変換が必要

### ファイル構成

```
src/bin/cli/
  main.rs         # エントリポイント + clap Args
  render.rs       # render_frame, render_group, render_path
  animation.rs    # アニメーション適用ロジック
  apng.rs         # APNG エンコード + unpremultiply
```

## Step D: 検証

```bash
# ライブラリテスト
cargo test

# 静止画 PNG 出力
cargo run --features cli --bin svg-cli -- -i tests/fixtures/checkout.svg -o /tmp/checkout.png

# APNG 出力
cargo run --features cli --bin svg-cli -- -i tests/fixtures/checkout.svg -o /tmp/checkout.apng --fps 30 --duration 3
```

## コミット戦略

1. `feat: integrate SMIL parser into parse.rs`
2. `test: add checkout.svg integration tests`
3. `feat: add svg-cli binary with PNG rendering support`
4. `feat: add APNG animated output to svg-cli`

## 重要ファイル一覧

| ファイル | 操作 | 内容 |
|---------|------|------|
| `src/parse.rs:47-49` | 修正 | SMIL スタブ → 実装 |
| `Cargo.toml` | 修正 | cli feature + 依存追加 |
| `tests/checkout.rs` | 新規 | 統合テスト |
| `src/bin/cli/main.rs` | 新規 | CLI エントリポイント |
| `src/bin/cli/render.rs` | 新規 | レンダリングエンジン |
| `src/bin/cli/animation.rs` | 新規 | アニメーション適用 |
| `src/bin/cli/apng.rs` | 新規 | APNG エンコード |

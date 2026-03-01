# Plan: Create README.md for OSS publication

## Context

svg-rs を OSS として公開するための英語 README.md を作成する。cargo publish は予定していないため、`cargo add --git` での利用を案内する。ライセンスセクションは不要。

## README.md の構成

以下の構成で英語の README.md を新規作成する:

### 1. Title & Description
- `svg-rs` のタイトル
- 一行説明: Zero-dependency SVG parser and SMIL animation evaluator for Rust

### 2. Features
- Zero external dependencies (core library)
- Full SVG path command support (M, L, H, V, C, S, Q, T, A, Z)
- SVG elements: rect, circle, ellipse, line, polyline, polygon, path
- Fill / stroke style parsing
- 2D affine transforms
- Path flattening (adaptive de Casteljau subdivision)
- Stroke-to-fill conversion (outline expansion)
- SMIL animation (animate, set, animateTransform)
- CSS Named Colors (148 colors)

### 3. Installation
- `cargo add --git https://github.com/ginokent/svg-rs.git`

### 4. Usage (Library API)
- `svg::parse()` — パース
- `svg::flatten()` — フラッテニング
- `svg::stroke_to_fill()` — ストローク変換
- `svg::evaluate()` — SMIL 評価
- 実際の lib.rs のシグネチャに基づいたコード例

### 5. CLI Tool
- インストール方法: `cargo install --git ... --features cli`
- 使用例 (PNG / APNG 出力)
- CLI オプション一覧

### 6. Architecture
- レイヤード構造の簡潔な図示
- 各モジュールの役割

### 7. Supported SVG Elements
- 対応要素一覧

## Files to create/modify

- `/Users/ginokent/go/src/github.com/ginokent/svg-rs/README.md` (新規作成)

## Verification

- README.md の markdown フォーマットが正しいこと
- コード例が lib.rs の実際の API と一致していること
- インストールコマンドが正しいこと

# svg-rs

Zero-dependency SVG parser and SMIL animation evaluator for Rust.

## Features

- Zero external dependencies (core library)
- Full SVG path command support (`M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, `Z` — both absolute and relative)
- SVG elements: `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>`
- Fill / stroke style parsing
- 2D affine transforms (`translate`, `rotate`, `scale`, `skewX`, `skewY`, `matrix`)
- Path flattening (adaptive de Casteljau subdivision)
- Stroke-to-fill conversion (outline expansion)
- SMIL animation (`<animate>`, `<set>`, `<animateTransform>`)
- CSS Named Colors (148 colors)

## Installation

```sh
cargo add --git https://github.com/ginokent/svg-rs.git
```

## Usage

### Parse an SVG

```rust
let svg_data = std::fs::read("input.svg").unwrap();
let doc = svg::parse(&svg_data).unwrap();
```

### Flatten paths

Convert cubic/quadratic bezier curves into polylines:

```rust
for node in &doc.root.children {
    if let svg::SvgNode::Path(path) = node {
        let polylines = svg::flatten(&path.segments, 0.25);
    }
}
```

### Stroke-to-fill conversion

Expand strokes into filled outlines:

```rust
if let Some(ref stroke_style) = path.stroke {
    let outline = svg::stroke_to_fill(&path.segments, stroke_style);
}
```

### Evaluate SMIL animations

```rust
let elapsed_secs = 1.5;
for anim in &doc.animations {
    if let Some(value) = svg::evaluate(anim, elapsed_secs) {
        // Apply animated value
    }
}
```

## CLI Tool

The `svg-cli` binary converts SVG files to PNG or APNG.

### Install

```sh
cargo install --git https://github.com/ginokent/svg-rs.git --features cli
```

### Examples

```sh
# Static PNG output
svg-cli -i input.svg -o output.png

# APNG animation output
svg-cli -i input.svg -o output.apng --fps 30 --duration 3

# Specify output size
svg-cli -i input.svg -o output.png --width 800 --height 600

# Render at a specific animation time
svg-cli -i input.svg -o output.png --time 1.5
```

### Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--input` | `-i` | *required* | Input SVG file path |
| `--output` | `-o` | *required* | Output file path (`.png` or `.apng`) |
| `--fps` | | `30` | APNG frame rate |
| `--duration` | | `3.0` | APNG animation duration in seconds |
| `--width` | | auto | Output width (derived from viewBox if omitted) |
| `--height` | | auto | Output height (derived from viewBox if omitted) |
| `--time` | | `0.0` | Animation time for static PNG output (seconds) |

Output format is determined by file extension: `.apng` produces animated PNG, everything else produces a static PNG.

## Architecture

```
svg::parse()          SVG bytes → SvgDocument
svg::flatten()        PathSegment[] → polylines
svg::stroke_to_fill() PathSegment[] → outlined PathSegment[]
svg::evaluate()       SmilAnimation → animated value at time t
```

### Modules

```
src/
├── lib.rs              Public API (parse, flatten, stroke_to_fill, evaluate)
├── types.rs            All public types (SvgDocument, SvgPath, PathSegment, ...)
├── parse.rs            Top-level parse orchestration
├── xml/
│   ├── tokenizer.rs    Zero-copy XML tokenizer
│   └── tree.rs         XML → tree builder
├── svg/
│   ├── elements.rs     SVG element → scene tree conversion
│   ├── path_data.rs    SVG path `d` attribute parser
│   ├── transform.rs    Transform attribute parser
│   ├── style.rs        Fill / stroke style parser
│   └── color.rs        CSS Named Colors + hex/rgb() parser
├── path/
│   ├── flatten.rs      Adaptive bezier flattening
│   └── stroke.rs       Stroke → fill outline expansion
└── smil/
    ├── parser.rs       SMIL attribute parser
    └── evaluator.rs    Animation value interpolation
```

## Supported SVG Elements

| Element | Attributes |
|---------|-----------|
| `<svg>` | `viewBox`, `width`, `height` |
| `<g>` | `transform`, `opacity`, `id` |
| `<rect>` | `x`, `y`, `width`, `height`, `rx`, `ry` |
| `<circle>` | `cx`, `cy`, `r` |
| `<ellipse>` | `cx`, `cy`, `rx`, `ry` |
| `<line>` | `x1`, `y1`, `x2`, `y2` |
| `<polyline>` | `points` |
| `<polygon>` | `points` |
| `<path>` | `d` |
| `<animate>` | `attributeName`, `from`, `to`, `dur`, `begin`, `repeatCount`, `fill` |
| `<set>` | `attributeName`, `to`, `begin`, `dur` |
| `<animateTransform>` | `attributeName`, `type`, `from`, `to`, `dur`, `begin`, `repeatCount` |

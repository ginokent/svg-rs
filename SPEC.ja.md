# `svg` クレート 仕様書

## 概要

SVG ファイルを GPU レンダリング用のデータ構造にパースし、SMIL アニメーションの評価機能を提供するライブラリクレート。外部依存ゼロ。特定のレンダリングバックエンドに依存しない汎用設計。

## 要件

- Rust, `edition = "2021"`
- 外部クレート依存なし (`std` のみ使用)
- クレート名: `svg`
- `#[derive(Debug, Clone, PartialEq)]` を全公開型に付与

## ディレクトリ構造

```
svg/
  Cargo.toml
  src/
    lib.rs              # 公開 API の re-export
    types.rs            # 公開データ型の定義
    parse.rs            # parse() エントリポイント
    xml/
      mod.rs            # XML パーサーモジュール
      tokenizer.rs      # XML トークナイザー (バイト列 → トークン)
      tree.rs           # XML ツリー構築
    svg/
      mod.rs            # SVG 変換モジュール
      elements.rs       # SVG 要素 → PathSegment 変換
      path_data.rs      # SVG path `d` 属性パーサー
      transform.rs      # transform 属性パーサー
      color.rs          # 色のパース (hex, rgb, rgba, named)
      style.rs          # fill/stroke 属性パース
    path/
      mod.rs            # パス処理ユーティリティ
      flatten.rs        # cubic bezier → 折れ線フラッテニング
      stroke.rs         # ストローク → フィルパス変換
    smil/
      mod.rs            # SMIL モジュール
      parser.rs         # SMIL 要素のパース
      evaluator.rs      # アニメーション値の計算
```

## 公開 API

### エントリポイント (`lib.rs`)

```rust
/// SVG バイト列をパースして SvgDocument を返す。
pub fn parse(data: &[u8]) -> Result<SvgDocument, SvgError>;

/// cubic bezier パスを折れ線にフラッテニングする。
/// tolerance はピクセル単位の近似誤差許容値 (例: 0.25)。
pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<(f32, f32)>>;

/// ストロークパスをフィルパス (アウトライン) に変換する。
pub fn stroke_to_fill(segments: &[PathSegment], style: &StrokeStyle) -> Vec<PathSegment>;

/// SMIL アニメーションの現在値を計算する。
/// elapsed_secs はアニメーション開始からの経過秒数。
/// アニメーションが begin 前なら None を返す。
pub fn evaluate(animation: &SmilAnimation, elapsed_secs: f64) -> Option<SmilValue>;

// types.rs の全公開型を re-export
pub use types::*;
```

### データ型 (`types.rs`)

```rust
// ============================================
// ドキュメント
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub struct SvgDocument {
    pub view_box: ViewBox,
    pub root: SvgGroup,
    pub animations: Vec<SmilAnimation>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// ============================================
// シーンツリー
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub struct SvgGroup {
    pub id: Option<String>,
    pub transform: Affine2D,
    pub opacity: f32,
    pub children: Vec<SvgNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SvgNode {
    Group(SvgGroup),
    Path(SvgPath),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SvgPath {
    pub id: Option<String>,
    pub segments: Vec<PathSegment>,
    pub fill: Option<FillStyle>,
    pub stroke: Option<StrokeStyle>,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillStyle {
    pub color: Color,
    pub rule: FillRule,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub dash: Option<DashPattern>,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashPattern {
    pub array: Vec<f32>,
    pub offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule { NonZero, EvenOdd }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap { Butt, Round, Square }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin { Miter, Round, Bevel }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

// ============================================
// パスデータ
// ============================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicTo {
        ctrl1_x: f32, ctrl1_y: f32,
        ctrl2_x: f32, ctrl2_y: f32,
        end_x: f32, end_y: f32,
    },
    Close,
}

// ============================================
// Transform
// ============================================

/// 2D アフィン変換行列
/// | a c e |
/// | b d f |
/// | 0 0 1 |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2D {
    pub a: f32, pub b: f32,
    pub c: f32, pub d: f32,
    pub e: f32, pub f: f32,
}

impl Affine2D {
    pub fn identity() -> Self;
    pub fn translate(tx: f32, ty: f32) -> Self;
    pub fn rotate(angle_deg: f32) -> Self;
    pub fn rotate_around(angle_deg: f32, cx: f32, cy: f32) -> Self;
    pub fn scale(sx: f32, sy: f32) -> Self;
    pub fn skew_x(angle_deg: f32) -> Self;
    pub fn skew_y(angle_deg: f32) -> Self;
    pub fn multiply(&self, other: &Self) -> Self;
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32);
}

// ============================================
// SMIL アニメーション
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub struct SmilAnimation {
    /// ターゲット要素の ID。子要素として配置されている場合は親要素の ID。
    pub target_id: String,
    pub kind: SmilKind,
    /// アニメーション対象の属性名 (例: "opacity", "transform", "fill", "stroke-dashoffset")
    pub attribute: String,
    /// パース済みの値リスト。from/to/by/values から正規化。
    pub values: Vec<SmilValue>,
    pub key_times: Option<Vec<f64>>,
    pub key_splines: Option<Vec<CubicBezier1D>>,
    pub timing: SmilTiming,
    pub calc_mode: CalcMode,
    pub additive: Additive,
    pub fill_mode: FillMode,
    /// AnimateTransform の場合の変換タイプ
    pub transform_type: Option<TransformType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmilTiming {
    pub begin: f64,          // 開始オフセット (秒)
    pub duration: f64,       // 持続時間 (秒)
    pub repeat_count: RepeatCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmilKind { Animate, Set, AnimateTransform }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatCount { Definite(f64), Indefinite }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcMode { Linear, Discrete, Spline, Paced }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Additive { Replace, Sum }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode { Freeze, Remove }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType { Translate, Rotate, Scale, SkewX, SkewY }

#[derive(Debug, Clone, PartialEq)]
pub enum SmilValue {
    Number(f64),
    NumberPair(f64, f64),
    NumberTriple(f64, f64, f64),
    Color(Color),
}

/// keySplines の 1 区間を表す 1D 三次ベジェ曲線の制御点。
/// (0,0) と (1,1) を端点とし、(x1,y1) と (x2,y2) が制御点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier1D {
    pub x1: f64, pub y1: f64,
    pub x2: f64, pub y2: f64,
}

// ============================================
// エラー
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub enum SvgError {
    /// XML の構文エラー
    XmlParse(String),
    /// SVG の構造エラー (必須属性の欠如等)
    InvalidSvg(String),
    /// パスデータのパースエラー
    InvalidPathData(String),
}

impl std::fmt::Display for SvgError { ... }
impl std::error::Error for SvgError {}
```

## 各モジュールの実装仕様

### 1. XML パーサー (`xml/`)

**目的**: SVG XML を内部ツリー構造にパースする。公開 API には露出しない。

**要件:**
- UTF-8 バイト列を入力として XML 要素ツリーを構築
- 要素ノード (タグ名、属性、子要素) とテキストノードを区別
- 自己閉じ要素 (`<br/>`) に対応
- 基本エンティティ参照の解決: `&amp;` → `&`, `&lt;` → `<`, `&gt;` → `>`, `&quot;` → `"`, `&apos;` → `'`
- 数値文字参照の解決: `&#123;` (10 進) → 対応する Unicode 文字、`&#x41;` (16 進) → 対応する Unicode 文字
- 名前空間の処理: `xmlns` / `xmlns:prefix` 属性を認識し、プレフィックスを除去してローカル名を取得
  - 例: `xlink:href` → ローカル名 `href`
- コメント (`<!-- -->`) はスキップ
- `<?xml ... ?>` 宣言はスキップ
- `<!DOCTYPE ...>` はスキップ
- CDATA セクション (`<![CDATA[...]]>`) は最低限テキストとして読めればよい

**内部 API:**
```rust
struct XmlDocument {
    root: XmlElement,
}

struct XmlElement {
    tag: String,               // ローカル名
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
}

enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    fn attribute(&self, name: &str) -> Option<&str>;
    fn children_elements(&self) -> impl Iterator<Item = &XmlElement>;
}

fn parse_xml(data: &[u8]) -> Result<XmlDocument, String>;
```

### 2. SVG 要素変換 (`svg/elements.rs`)

**目的**: SVG ジオメトリ要素を `Vec<PathSegment>` に変換する。

**変換ルール:**

#### `<rect>`
```
入力: x, y, width, height, rx (optional), ry (optional)
出力:
  - rx=0, ry=0: 4 つの LineTo + Close
  - rx>0 or ry>0: 4 辺の LineTo + 4 角の CubicTo (円弧近似) + Close

角丸近似: 四分円弧を cubic bezier で近似する定数 k = 0.5522847498
  CubicTo(r*k, 0, r, r*(1-k), r, r)  ← 1 角分
```

#### `<circle>`
```
入力: cx, cy, r
出力: 4 つの CubicTo で円を近似 (四分円弧 × 4)

円の cubic bezier 近似:
  k = 0.5522847498  (= 4/3 * (sqrt(2) - 1))
  MoveTo(cx + r, cy)
  CubicTo(cx + r, cy + r*k,  cx + r*k, cy + r,  cx, cy + r)   // 右上→上
  CubicTo(cx - r*k, cy + r,  cx - r, cy + r*k,  cx - r, cy)   // 上→左
  CubicTo(cx - r, cy - r*k,  cx - r*k, cy - r,  cx, cy - r)   // 左→下
  CubicTo(cx + r*k, cy - r,  cx + r, cy - r*k,  cx + r, cy)   // 下→右
  Close
```

#### `<ellipse>`
```
入力: cx, cy, rx, ry
出力: circle と同じだが rx, ry を使う
```

#### `<line>`
```
入力: x1, y1, x2, y2
出力: MoveTo(x1, y1), LineTo(x2, y2)
```

#### `<polyline>`
```
入力: points="x1,y1 x2,y2 x3,y3 ..."
出力: MoveTo(x1, y1), LineTo(x2, y2), LineTo(x3, y3), ...
```

#### `<polygon>`
```
入力: points="x1,y1 x2,y2 x3,y3 ..."
出力: MoveTo(x1, y1), LineTo(x2, y2), LineTo(x3, y3), ..., Close
```

### 3. SVG path `d` 属性パーサー (`svg/path_data.rs`)

**目的**: SVG path の `d` 属性文字列を `Vec<PathSegment>` にパースする。

**正規化ルール**: 全てのコマンドを `MoveTo`, `LineTo`, `CubicTo`, `Close` の 4 種に変換する。

| SVG コマンド | 正規化先 | 変換方法 |
|-------------|---------|---------|
| `M x y` / `m dx dy` | `MoveTo` | 絶対座標に変換。M の後続座標は暗黙の L として扱う |
| `L x y` / `l dx dy` | `LineTo` | 絶対座標に変換 |
| `H x` / `h dx` | `LineTo` | y は現在位置から継承 |
| `V y` / `v dy` | `LineTo` | x は現在位置から継承 |
| `C x1 y1 x2 y2 x y` / `c ...` | `CubicTo` | 絶対座標に変換 |
| `S x2 y2 x y` / `s ...` | `CubicTo` | ctrl1 は前回の ctrl2 の反射。前回が C/S でなければ ctrl1 = 現在位置 |
| `Q x1 y1 x y` / `q ...` | `CubicTo` | **二次→三次ベジェ昇格** (下記参照) |
| `T x y` / `t ...` | `CubicTo` | ctrl は前回の Q/T の反射。二次→三次昇格 |
| `A rx ry rotation large-arc sweep x y` / `a ...` | `CubicTo` × N | **楕円弧→三次ベジェ近似** (下記参照) |
| `Z` / `z` | `Close` | |

#### 二次ベジェ → 三次ベジェ昇格

```
二次ベジェ: P0 → Q → P1 (制御点 1 つ)
三次ベジェ: P0 → C1 → C2 → P1 (制御点 2 つ)

変換:
  C1 = P0 + 2/3 * (Q - P0)
  C2 = P1 + 2/3 * (Q - P1)
```

#### 楕円弧 → 三次ベジェ近似

```
SVG arc: (rx, ry, x_rotation, large_arc_flag, sweep_flag, x, y)

変換手順:
1. エンドポイントパラメータ → 中心パラメータ変換
   (SVG spec §F.6.5 / §F.6.6 のアルゴリズム)
2. 弧を 90° 以下の区間に分割
3. 各区間を cubic bezier で近似:
   α = 4/3 * tan(θ/4)  (θ は区間の角度)
```

**パース時の注意:**
- 数値の区切りはスペース、カンマ、またはマイナス記号 (例: `10-20` = `10, -20`)
- 暗黙の繰り返し: `L 10 20 30 40` = `L 10 20 L 30 40`
- 小文字コマンドは現在位置からの相対座標
- `M` の後続座標は暗黙の `L`、`m` の後続座標は暗黙の `l`

### 4. transform パーサー (`svg/transform.rs`)

**目的**: SVG `transform` 属性を `Affine2D` にパースする。

**対応する transform 関数:**

```
translate(tx)         → Affine2D { a:1, b:0, c:0, d:1, e:tx, f:0 }
translate(tx, ty)     → Affine2D { a:1, b:0, c:0, d:1, e:tx, f:ty }
scale(s)              → Affine2D { a:s, b:0, c:0, d:s, e:0, f:0 }
scale(sx, sy)         → Affine2D { a:sx, b:0, c:0, d:sy, e:0, f:0 }
rotate(deg)           → Affine2D { a:cos, b:sin, c:-sin, d:cos, e:0, f:0 }
rotate(deg, cx, cy)   → translate(cx,cy) * rotate(deg) * translate(-cx,-cy)
skewX(deg)            → Affine2D { a:1, b:0, c:tan(deg), d:1, e:0, f:0 }
skewY(deg)            → Affine2D { a:1, b:tan(deg), c:0, d:1, e:0, f:0 }
matrix(a,b,c,d,e,f)  → Affine2D { a, b, c, d, e, f }
```

**複数 transform の連結:**
`transform="translate(10,20) rotate(45)"` → `translate(10,20).multiply(rotate(45))`

**行列の乗算 (multiply):**
```
[a1 c1 e1]   [a2 c2 e2]   [a1*a2+c1*b2  a1*c2+c1*d2  a1*e2+c1*f2+e1]
[b1 d1 f1] × [b2 d2 f2] = [b1*a2+d1*b2  b1*c2+d1*d2  b1*e2+d1*f2+f1]
[0  0  1 ]   [0  0  1 ]   [0            0             1              ]
```

### 5. 色パーサー (`svg/color.rs`)

**対応フォーマット:**

| フォーマット | 例 | パース方法 |
|------------|---|---------|
| `#RGB` | `#F00` | 各桁を 2 回繰り返し: `#FF0000` |
| `#RRGGBB` | `#FF8800` | 直接パース |
| `#RRGGBBAA` | `#FF880080` | 直接パース |
| `rgb(r, g, b)` | `rgb(255, 128, 0)` | 整数値 0-255 |
| `rgb(r%, g%, b%)` | `rgb(100%, 50%, 0%)` | パーセント → 0-255 |
| `rgba(r, g, b, a)` | `rgba(0,0,0,0.08)` | a は 0.0-1.0 → 0-255 |
| `none` | `none` | → `None` (fill/stroke なし) |
| CSS Named Color | `red`, `blue` 等 | テーブル参照 (148 色) |

**CSS Named Colors テーブル** (148 色): `aliceblue`, `antiquewhite`, ..., `yellowgreen`
CSS Named Colors として定義される 148 色すべてに対応する。色名から RGB 値への変換テーブルを定数配列として保持する。

### 6. fill/stroke 属性パース (`svg/style.rs`)

**fill 関連属性:**
- `fill`: 色またはなし (デフォルト: `#000000`)
- `fill-rule`: `nonzero` (デフォルト) | `evenodd`
- `fill-opacity`: 0.0-1.0 (デフォルト: 1.0)

**stroke 関連属性:**
- `stroke`: 色またはなし (デフォルト: `none`)
- `stroke-width`: 数値 (デフォルト: 1.0)
- `stroke-linecap`: `butt` (デフォルト) | `round` | `square`
- `stroke-linejoin`: `miter` (デフォルト) | `round` | `bevel`
- `stroke-dasharray`: カンマ区切りの数値リスト | `none`
- `stroke-dashoffset`: 数値 (デフォルト: 0.0)
- `stroke-opacity`: 0.0-1.0 (デフォルト: 1.0)

**opacity 属性:**
- `opacity`: 要素全体の不透明度 (デフォルト: 1.0)。fill-opacity/stroke-opacity とは独立。

### 7. パスフラッテニング (`path/flatten.rs`)

**目的**: `Vec<PathSegment>` (cubic bezier) を折れ線 `Vec<(f32, f32)>` に変換する。

**アルゴリズム**: 適応的 de Casteljau 分割

```
flatten_cubic(p0, p1, p2, p3, tolerance, output):
  // フラットネステスト: 制御点が直線からどれだけ離れているか
  d1 = 点 p1 から直線 p0-p3 への距離
  d2 = 点 p2 から直線 p0-p3 への距離
  if max(d1, d2) <= tolerance:
    output.push(p3)  // 十分フラット、直線で近似
    return

  // de Casteljau 分割 (t=0.5)
  (left, right) = split_cubic_at_half(p0, p1, p2, p3)
  flatten_cubic(left, tolerance, output)
  flatten_cubic(right, tolerance, output)
```

**de Casteljau 分割 (t=0.5):**
```
p01 = (p0 + p1) / 2
p12 = (p1 + p2) / 2
p23 = (p2 + p3) / 2
p012 = (p01 + p12) / 2
p123 = (p12 + p23) / 2
p0123 = (p012 + p123) / 2  ← 分割点

left  = (p0, p01, p012, p0123)
right = (p0123, p123, p23, p3)
```

**MoveTo/Close の扱い:**
- `MoveTo(x, y)` → サブパスの開始。新しいサブパスを開始し、`output.push((x, y))`
- `Close` → 最後の `MoveTo` の点を `output.push()` し、サブパスを閉じる
- 戻り値は `Vec<Vec<(f32, f32)>>` 型で、サブパスごとに分割された折れ線を返す。各サブパスは閉じたポリゴン (最初と最後の点が同一) または開いた折れ線

### 8. ストローク → フィルパス変換 (`path/stroke.rs`)

**目的**: ストロークをアウトラインのフィルパスに変換する。

**概要:**
1. パスの各セグメントに対して、stroke-width / 2 だけ法線方向にオフセットした外形パスと内形パスを生成
2. LineCap でパス端点を処理
3. LineJoin で屈折点を処理
4. DashPattern を適用 (ダッシュ区間のみアウトラインを生成)

**アルゴリズム:**

#### 法線オフセット
```
各セグメントの始点・終点における法線ベクトルを計算:
  LineTo(x0,y0 → x1,y1):
    dx = x1 - x0, dy = y1 - y0
    len = sqrt(dx*dx + dy*dy)
    normal = (-dy/len, dx/len)  // 左側法線

外形点 = 元の点 + normal * (width / 2)
内形点 = 元の点 - normal * (width / 2)
```

#### LineCap
```
Butt:   端点で直角カット (追加処理なし)
Round:  端点に半円を追加 (4 つの CubicTo で近似)
Square: 端点を stroke-width/2 だけ延長した直角カット
```

#### LineJoin
```
Miter:  外角の延長線の交点まで延長 (miter-limit 超過時は Bevel にフォールバック)
Round:  外角に円弧を追加 (CubicTo で近似)
Bevel:  外角を直線で面取り
```

#### DashPattern 適用
```
1. パスの全長を計算
2. dasharray パターンに従って「描画区間」と「非描画区間」を交互に決定
3. dashoffset だけオフセット
4. 描画区間のみを抽出してアウトライン化
```

**対応範囲:**
- LineTo (折れ線) のストローク変換
- CubicTo のストローク変換 (曲線のオフセット近似)
- DashPattern 適用

### 9. SMIL パーサー (`smil/parser.rs`)

**目的**: XML ツリーから SMIL アニメーション要素を抽出し `Vec<SmilAnimation>` を返す。

**対象要素:**
- `<animate>` → `SmilKind::Animate`
- `<set>` → `SmilKind::Set`
- `<animateTransform>` → `SmilKind::AnimateTransform`
- `<animateMotion>` → SMIL タグとして認識しシーンツリー構築時にスキップするが、`SmilAnimation` としてのパースは行わない

**ターゲット要素の特定:**
1. SMIL 要素が子要素として配置: 親要素がターゲット
   ```xml
   <circle id="c1"><animate attributeName="cx" .../></circle>
   ```
   → `target_id = "c1"`

2. `href` / `xlink:href` 属性で参照:
   ```xml
   <animate href="#c1" attributeName="cx" .../>
   ```
   → `target_id = "c1"`

3. 親要素に id がない場合: 自動生成 ID を付与 (`__smil_target_0`, `__smil_target_1`, ...)

**属性パース:**

| 属性 | パース方法 |
|------|---------|
| `attributeName` | そのまま文字列 |
| `from`, `to` | SmilValue にパース (型は attributeName に依存) |
| `by` | `from` + `by` で `to` を計算 |
| `values` | セミコロン区切りで SmilValue のリストにパース |
| `keyTimes` | セミコロン区切りで f64 のリスト。values と同じ長さ |
| `keySplines` | セミコロン区切りで CubicBezier1D のリスト。(values の長さ - 1) 個 |
| `begin` | 秒数にパース。`"2s"` → 2.0, `"500ms"` → 0.5。デフォルト: 0.0 |
| `dur` | 秒数にパース |
| `repeatCount` | `"indefinite"` → Indefinite, 数値 → Definite(n) |
| `fill` | `"freeze"` → Freeze, `"remove"` → Remove。デフォルト: Remove |
| `calcMode` | `"linear"`, `"discrete"`, `"spline"`, `"paced"` |
| `additive` | `"replace"` (デフォルト), `"sum"` |
| `type` (animateTransform) | `"translate"`, `"rotate"`, `"scale"`, `"skewX"`, `"skewY"` |

**SmilValue のパース (attributeName に応じて):**

| attributeName | SmilValue 型 | パース例 |
|--------------|-------------|---------|
| `opacity`, `stroke-dashoffset`, 数値属性 | `Number` | `"0.5"` → `Number(0.5)` |
| `fill`, `stroke` (色) | `Color` | `"#FF0000"` → `Color(255,0,0,255)` |
| `transform` (type=translate) | `NumberPair` | `"10 20"` → `NumberPair(10.0, 20.0)` |
| `transform` (type=scale) | `Number` or `NumberPair` | `"1.5"` → `Number(1.5)` |
| `transform` (type=rotate) | `Number` or `NumberTriple` | `"45"` → `Number(45.0)`, `"45 50 50"` → `NumberTriple(45.0, 50.0, 50.0)` |

**values パース例 (checkout.svg から):**
```
values="0; 1; 1; 0; 0; 1; 1"
→ [Number(0.0), Number(1.0), Number(1.0), Number(0.0), Number(0.0), Number(1.0), Number(1.0)]

values="0 -80; 0 -80; 0 0; 0 -12; 0 0; 0 0"
→ [NumberPair(0,-80), NumberPair(0,-80), NumberPair(0,0), NumberPair(0,-12), NumberPair(0,0), NumberPair(0,0)]

keySplines="0 0 0.58 1; 0 0 1 1; 0.42 0 0.58 1; 0 0 1 1; 0 0 1 1; 0 0 1 1"
→ [CubicBezier1D{0,0,0.58,1}, CubicBezier1D{0,0,1,1}, ...]
```

### 10. SMIL エバリュエーター (`smil/evaluator.rs`)

**目的**: 経過時間から現在のアニメーション値を計算する。

**アルゴリズム:**

```rust
pub fn evaluate(anim: &SmilAnimation, elapsed: f64) -> Option<SmilValue> {
    let t = &anim.timing;

    // 1. begin 前なら None
    if elapsed < t.begin {
        return None;
    }

    let local_time = elapsed - t.begin;

    // 2. アクティブ期間の計算
    let active_duration = match t.repeat_count {
        Indefinite => f64::INFINITY,
        Definite(n) => t.duration * n,
    };

    // 3. アクティブ期間超過
    if local_time > active_duration {
        return match anim.fill_mode {
            Freeze => Some(anim.values.last()?.clone()),
            Remove => None,
        };
    }

    // 4. 繰り返し内の進行度
    let repeat_time = local_time % t.duration;
    let progress = repeat_time / t.duration;  // 0.0 ~ 1.0

    // 5. keyTimes がある場合、区間を特定
    let (segment_index, segment_progress) = if let Some(kt) = &anim.key_times {
        find_segment(kt, progress)
    } else {
        // values を等間隔で分割
        let n = anim.values.len() - 1;
        let raw = progress * n as f64;
        let idx = (raw as usize).min(n - 1);
        (idx, raw - idx as f64)
    };

    // 6. calcMode に応じた補間
    let t_interpolated = match anim.calc_mode {
        Linear => segment_progress,
        Discrete => 0.0,  // 区間の開始値を使用
        Spline => {
            // keySplines による三次ベジェ補間
            let spline = &anim.key_splines.as_ref()?[segment_index];
            solve_cubic_bezier_1d(spline, segment_progress)
        },
        Paced => segment_progress,  // 簡略化: linear と同じ
    };

    // 7. 値の補間
    let v0 = &anim.values[segment_index];
    let v1 = &anim.values[segment_index + 1];

    if anim.calc_mode == CalcMode::Discrete {
        return Some(v0.clone());
    }

    Some(interpolate_smil_value(v0, v1, t_interpolated))
}
```

#### keySplines の三次ベジェ補間

`solve_cubic_bezier_1d(spline, t)` は、ベジェ曲線 `B(u) = (x(u), y(u))` で `x(u) = t` となる `u` を求め、`y(u)` を返す。

```
ベジェ曲線: (0,0) → (x1,y1) → (x2,y2) → (1,1)

x(u) = 3*(1-u)^2*u*x1 + 3*(1-u)*u^2*x2 + u^3

x(u) = t を満たす u を二分探索 (Newton 法でも可) で求め、y(u) を返す。
```

#### SmilValue の補間

```rust
fn interpolate_smil_value(v0: &SmilValue, v1: &SmilValue, t: f64) -> SmilValue {
    match (v0, v1) {
        (Number(a), Number(b)) => Number(a + (b - a) * t),
        (NumberPair(a1,a2), NumberPair(b1,b2)) => NumberPair(
            a1 + (b1 - a1) * t,
            a2 + (b2 - a2) * t,
        ),
        (NumberTriple(a1,a2,a3), NumberTriple(b1,b2,b3)) => NumberTriple(
            a1 + (b1 - a1) * t,
            a2 + (b2 - a2) * t,
            a3 + (b3 - a3) * t,
        ),
        (Color(c0), Color(c1)) => Color(Color {
            r: lerp_u8(c0.r, c1.r, t),
            g: lerp_u8(c0.g, c1.g, t),
            b: lerp_u8(c0.b, c1.b, t),
            a: lerp_u8(c0.a, c1.a, t),
        }),
        _ => v0.clone(),  // 型不一致時はフォールバック
    }
}
```

## テスト

### ユニットテスト

各モジュールに `#[cfg(test)] mod tests` を設ける:

1. **XML パーサー**: 基本的な XML のパース、名前空間処理、エンティティ参照
2. **path `d` パーサー**: 各コマンドの正規化、相対→絶対変換、暗黙繰り返し
3. **transform パーサー**: 各関数の行列生成、複数 transform の連結
4. **色パーサー**: hex, rgb(), rgba(), named colors
5. **フラッテニング**: 直線 (変化なし)、曲線 (分割)、tolerance による精度
6. **SMIL パーサー**: animate/set/animateTransform の属性パース
7. **SMIL エバリュエーター**: 線形補間、discrete、keySplines、repeatCount、freeze/remove

### 統合テスト

`tests/` ディレクトリ:

```rust
// tests/checkout.rs

fn checkout_svg() -> &'static [u8] {
    include_bytes!("fixtures/checkout.svg")
}

#[test]
fn test_checkout_parse_viewbox() {
    let doc = svg::parse(checkout_svg()).unwrap();
    assert_eq!(doc.view_box.width, 400.0);
    assert_eq!(doc.view_box.height, 400.0);
}

#[test]
fn test_checkout_has_animations() {
    let doc = svg::parse(checkout_svg()).unwrap();
    assert!(doc.animations.len() >= 15);
}

#[test]
fn test_checkout_has_paths() {
    let doc = svg::parse(checkout_svg()).unwrap();
    let path_count = count_paths(&doc.root);
    assert!(path_count >= 10);
}
```

## モジュール依存関係

| モジュール | 依存先 |
|-----------|--------|
| **types.rs** | なし (他の全モジュールがこれに依存) |
| **xml/** | types.rs |
| **svg/path_data.rs** | types.rs |
| **svg/transform.rs** | types.rs |
| **svg/color.rs** | types.rs |
| **svg/style.rs** | types.rs, svg/color.rs |
| **svg/elements.rs** | types.rs, svg/path_data.rs, svg/transform.rs |
| **parse.rs** | types.rs, xml/, svg/, smil/ |
| **path/flatten.rs** | types.rs |
| **path/stroke.rs** | types.rs, path/flatten.rs |
| **smil/parser.rs** | types.rs, svg/color.rs |
| **smil/evaluator.rs** | types.rs |

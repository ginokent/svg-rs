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
    pub visibility: bool,
    pub clip_path: Option<Vec<PathSegment>>,
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
    pub visibility: bool,
    pub clip_path: Option<Vec<PathSegment>>,
}

/// 塗りの種類。色またはグラデーション参照。
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillStyle {
    pub paint: Paint,
    pub rule: FillRule,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    pub paint: Paint,
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
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

// ============================================
// グラデーション
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stops: Vec<GradientStop>,
    pub spread: SpreadMethod,
    pub transform: Affine2D,
    pub units: GradientUnits,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub fx: f32,
    pub fy: f32,
    pub stops: Vec<GradientStop>,
    pub spread: SpreadMethod,
    pub transform: Affine2D,
    pub units: GradientUnits,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    pub offset: f32, // 0.0-1.0
    pub color: Color,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMethod {
    Pad,
    Reflect,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientUnits {
    UserSpaceOnUse,
    ObjectBoundingBox,
}

// ============================================
// パスデータ
// ============================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicTo {
        ctrl1_x: f32,
        ctrl1_y: f32,
        ctrl2_x: f32,
        ctrl2_y: f32,
        end_x: f32,
        end_y: f32,
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
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Affine2D {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    pub fn rotate(angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn rotate_around(angle_deg: f32, cx: f32, cy: f32) -> Self {
        // translate(cx,cy) * rotate(deg) * translate(-cx,-cy)
        let t1 = Self::translate(cx, cy);
        let r = Self::rotate(angle_deg);
        let t2 = Self::translate(-cx, -cy);
        t1.multiply(&r).multiply(&t2)
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn skew_x(angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        Self {
            a: 1.0,
            b: 0.0,
            c: rad.tan(),
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn skew_y(angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        Self {
            a: 1.0,
            b: rad.tan(),
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// 行列の乗算:
    /// [a1 c1 e1]   [a2 c2 e2]   [a1*a2+c1*b2  a1*c2+c1*d2  a1*e2+c1*f2+e1]
    /// [b1 d1 f1] × [b2 d2 f2] = [b1*a2+d1*b2  b1*c2+d1*d2  b1*e2+d1*f2+f1]
    /// [0  0  1 ]   [0  0  1 ]   [0            0             1              ]
    pub fn multiply(&self, other: &Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
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
    pub begin: f64,
    pub duration: f64,
    pub repeat_count: RepeatCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmilKind {
    Animate,
    Set,
    AnimateTransform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatCount {
    Definite(f64),
    Indefinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcMode {
    Linear,
    Discrete,
    Spline,
    Paced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Additive {
    Replace,
    Sum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    Freeze,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    Translate,
    Rotate,
    Scale,
    SkewX,
    SkewY,
}

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
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
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

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgError::XmlParse(msg) => write!(f, "XML parse error: {}", msg),
            SvgError::InvalidSvg(msg) => write!(f, "Invalid SVG: {}", msg),
            SvgError::InvalidPathData(msg) => write!(f, "Invalid path data: {}", msg),
        }
    }
}

impl std::error::Error for SvgError {}

//! SMIL アニメーション適用モジュール。
//!
//! 指定時刻での全アニメーションの評価結果をキャッシュし、
//! レンダリング時に要素ごとのプロパティ解決を提供する。

use std::collections::HashMap;
use svg::*;

/// フレーム描画時のアニメーション状態。
/// 全アニメーションを elapsed_secs で事前評価し、結果をキャッシュする。
pub struct AnimationState {
    /// (target_id, attribute) → 評価結果リスト
    /// 同一 (target_id, attribute) に複数アニメーションが存在しうるため Vec で保持。
    values: HashMap<(String, String), Vec<EvaluatedAnim>>,
}

struct EvaluatedAnim {
    value: SmilValue,
    additive: Additive,
    transform_type: Option<TransformType>,
}

impl AnimationState {
    /// 全アニメーションを elapsed_secs で評価し、AnimationState を構築する。
    pub fn new(animations: &[SmilAnimation], elapsed_secs: f64) -> Self {
        let mut values: HashMap<(String, String), Vec<EvaluatedAnim>> = HashMap::new();

        for anim in animations {
            if let Some(val) = svg::evaluate(anim, elapsed_secs) {
                let key = (anim.target_id.clone(), anim.attribute.clone());
                values.entry(key).or_default().push(EvaluatedAnim {
                    value: val,
                    additive: anim.additive,
                    transform_type: anim.transform_type,
                });
            }
        }

        Self { values }
    }

    /// 要素の transform を解決する。
    /// アニメーションが存在する場合、additive に応じて base に合成または置換する。
    pub fn resolve_transform(&self, id: Option<&str>, base: &Affine2D) -> Affine2D {
        let id = match id {
            Some(id) => id,
            None => return *base,
        };

        let key = (id.to_string(), "transform".to_string());
        let anims = match self.values.get(&key) {
            Some(anims) => anims,
            None => return *base,
        };

        // 複数の animateTransform が存在する場合、順番に合成する。
        // additive=replace の場合は identity から開始、sum の場合は base から開始。
        let mut result = *base;
        for anim in anims {
            let anim_transform = smil_value_to_transform(&anim.value, anim.transform_type);
            match anim.additive {
                Additive::Replace => result = anim_transform,
                Additive::Sum => result = result.multiply(&anim_transform),
            }
        }
        result
    }

    /// 要素の opacity を解決する。
    pub fn resolve_opacity(&self, id: Option<&str>, base: f32) -> f32 {
        let id = match id {
            Some(id) => id,
            None => return base,
        };

        let key = (id.to_string(), "opacity".to_string());
        match self.values.get(&key) {
            Some(anims) => {
                // 最後の replace アニメーションの値を使用
                for anim in anims.iter().rev() {
                    if let SmilValue::Number(v) = &anim.value {
                        return *v as f32;
                    }
                }
                base
            }
            None => base,
        }
    }

    /// 要素の fill カラーを解決する。
    pub fn resolve_fill(&self, id: Option<&str>, base: &FillStyle) -> FillStyle {
        let id = match id {
            Some(id) => id,
            None => return base.clone(),
        };

        let key = (id.to_string(), "fill".to_string());
        match self.values.get(&key) {
            Some(anims) => {
                let mut result = base.clone();
                for anim in anims.iter().rev() {
                    if let SmilValue::Color(c) = &anim.value {
                        result.paint = svg::Paint::Color(*c);
                        return result;
                    }
                }
                result
            }
            None => base.clone(),
        }
    }

    /// 要素の stroke を解決する。
    pub fn resolve_stroke(&self, id: Option<&str>, base: &StrokeStyle) -> StrokeStyle {
        let id = match id {
            Some(id) => id,
            None => return base.clone(),
        };

        let key = (id.to_string(), "stroke".to_string());
        let mut result = base.clone();
        if let Some(anims) = self.values.get(&key) {
            for anim in anims.iter().rev() {
                if let SmilValue::Color(c) = &anim.value {
                    result.paint = svg::Paint::Color(*c);
                    break;
                }
            }
        }
        result
    }

    /// 要素の stroke-dashoffset を解決する。
    pub fn resolve_dash_offset(&self, id: Option<&str>, base: f32) -> f32 {
        let id = match id {
            Some(id) => id,
            None => return base,
        };

        let key = (id.to_string(), "stroke-dashoffset".to_string());
        match self.values.get(&key) {
            Some(anims) => {
                for anim in anims.iter().rev() {
                    if let SmilValue::Number(v) = &anim.value {
                        return *v as f32;
                    }
                }
                base
            }
            None => base,
        }
    }
}

/// SmilValue と TransformType から Affine2D を構築する。
///
/// 対応表:
///   Scale:     Number(n)        → scale(n, n)
///              NumberPair(x, y) → scale(x, y)
///   Translate: Number(n)        → translate(n, 0)
///              NumberPair(x, y) → translate(x, y)
///   Rotate:    Number(deg)             → rotate(deg)
///              NumberTriple(deg, cx, cy) → rotate_around(deg, cx, cy)
///   SkewX:     Number(deg)      → skew_x(deg)
///   SkewY:     Number(deg)      → skew_y(deg)
fn smil_value_to_transform(value: &SmilValue, transform_type: Option<TransformType>) -> Affine2D {
    let tt = match transform_type {
        Some(tt) => tt,
        None => return Affine2D::identity(),
    };

    match tt {
        TransformType::Scale => match value {
            SmilValue::Number(n) => Affine2D::scale(*n as f32, *n as f32),
            SmilValue::NumberPair(x, y) => Affine2D::scale(*x as f32, *y as f32),
            _ => Affine2D::identity(),
        },
        TransformType::Translate => match value {
            SmilValue::Number(n) => Affine2D::translate(*n as f32, 0.0),
            SmilValue::NumberPair(x, y) => Affine2D::translate(*x as f32, *y as f32),
            _ => Affine2D::identity(),
        },
        TransformType::Rotate => match value {
            SmilValue::Number(deg) => Affine2D::rotate(*deg as f32),
            SmilValue::NumberTriple(deg, cx, cy) => {
                Affine2D::rotate_around(*deg as f32, *cx as f32, *cy as f32)
            }
            _ => Affine2D::identity(),
        },
        TransformType::SkewX => match value {
            SmilValue::Number(deg) => Affine2D::skew_x(*deg as f32),
            _ => Affine2D::identity(),
        },
        TransformType::SkewY => match value {
            SmilValue::Number(deg) => Affine2D::skew_y(*deg as f32),
            _ => Affine2D::identity(),
        },
    }
}

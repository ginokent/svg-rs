use crate::types::*;

/// SMIL アニメーションの現在値を計算する。
///
/// `elapsed_secs` はドキュメント開始からの経過秒数。
/// アニメーションがまだ開始していない場合や、終了後に `FillMode::Remove` の場合は `None` を返す。
pub fn evaluate(anim: &SmilAnimation, elapsed_secs: f64) -> Option<SmilValue> {
    let t = &anim.timing;

    // エッジケース: 値リストが空の場合は常に None
    if anim.values.is_empty() {
        return None;
    }

    // エッジケース: 値が 1 つだけの場合は補間不要、その値を返す
    // (ただし begin 前は None)
    if anim.values.len() == 1 {
        if elapsed_secs < t.begin {
            return None;
        }
        return Some(anim.values[0].clone());
    }

    // 1. begin より前 → None
    if elapsed_secs < t.begin {
        return None;
    }

    let local_time = elapsed_secs - t.begin;

    // 2. アクティブ期間の算出
    let active_duration = match t.repeat_count {
        RepeatCount::Indefinite => f64::INFINITY,
        RepeatCount::Definite(n) => t.duration * n,
    };

    // エッジケース: duration == 0 の場合
    if t.duration <= 0.0 {
        return match anim.fill_mode {
            FillMode::Freeze => Some(anim.values.last()?.clone()),
            FillMode::Remove => None,
        };
    }

    // 3. アクティブ期間を過ぎた場合
    if local_time >= active_duration {
        return match anim.fill_mode {
            FillMode::Freeze => Some(anim.values.last()?.clone()),
            FillMode::Remove => None,
        };
    }

    // 4. リピートサイクル内の進行度を算出
    let repeat_time = local_time % t.duration;
    let progress = (repeat_time / t.duration).clamp(0.0, 1.0);

    // エッジケース: progress が丁度 1.0 の場合は最後の値を返す
    if (progress - 1.0).abs() < f64::EPSILON {
        return Some(anim.values.last()?.clone());
    }

    // 5. keyTimes または等間隔分割でセグメントとセグメント内進行度を決定
    let n = anim.values.len() - 1; // セグメント数
    let (segment_index, segment_progress) = if let Some(kt) = &anim.key_times {
        find_segment(kt, progress)
    } else {
        // 等間隔分割
        let raw = progress * n as f64;
        let idx = (raw as usize).min(n - 1);
        (idx, raw - idx as f64)
    };

    // 6. calcMode に応じた補間パラメータの変換
    let t_interpolated = match anim.calc_mode {
        CalcMode::Linear => segment_progress,
        CalcMode::Discrete => 0.0, // 常にセグメント開始値を使用
        CalcMode::Spline => {
            let splines = anim.key_splines.as_ref()?;
            let spline = splines.get(segment_index)?;
            solve_cubic_bezier_1d(spline, segment_progress)
        }
        CalcMode::Paced => segment_progress, // 簡略化: Linear と同じ
    };

    // 7. Discrete モードの場合はセグメント開始値をそのまま返す
    if anim.calc_mode == CalcMode::Discrete {
        return Some(anim.values[segment_index].clone());
    }

    // 8. 値の補間
    let v0 = &anim.values[segment_index];
    let v1 = &anim.values[segment_index + 1];
    Some(interpolate_smil_value(v0, v1, t_interpolated))
}

/// keyTimes 配列から、progress がどのセグメントに属するかを探す。
///
/// 返り値: (セグメントインデックス, セグメント内の正規化された進行度 0.0..1.0)
fn find_segment(key_times: &[f64], progress: f64) -> (usize, f64) {
    // keyTimes が空または 1 要素の場合のフォールバック
    if key_times.len() < 2 {
        return (0, progress);
    }

    // progress が属するセグメントを線形スキャンで探す
    // keyTimes[i] <= progress < keyTimes[i+1] となる i を探す
    let last_seg = key_times.len() - 2;
    for i in 0..last_seg {
        if progress < key_times[i + 1] {
            let seg_start = key_times[i];
            let seg_end = key_times[i + 1];
            let seg_duration = seg_end - seg_start;
            if seg_duration <= 0.0 {
                return (i, 0.0);
            }
            let seg_progress = ((progress - seg_start) / seg_duration).clamp(0.0, 1.0);
            return (i, seg_progress);
        }
    }

    // 最後のセグメント
    let seg_start = key_times[last_seg];
    let seg_end = key_times[last_seg + 1];
    let seg_duration = seg_end - seg_start;
    if seg_duration <= 0.0 {
        return (last_seg, 1.0);
    }
    let seg_progress = ((progress - seg_start) / seg_duration).clamp(0.0, 1.0);
    (last_seg, seg_progress)
}

/// 1D 三次ベジェ曲線を解く。
///
/// 制御点 P0=(0,0), P1=(x1,y1), P2=(x2,y2), P3=(1,1) のベジェ曲線において、
/// x(u) = t となる u を求め、y(u) を返す。
///
/// x(u) = 3(1-u)^2 * u * x1 + 3(1-u) * u^2 * x2 + u^3
/// y(u) = 3(1-u)^2 * u * y1 + 3(1-u) * u^2 * y2 + u^3
///
/// 二分探索で u を求める。
fn solve_cubic_bezier_1d(spline: &CubicBezier1D, t: f64) -> f64 {
    // エッジケース
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // 二分探索で x(u) = t となる u を見つける
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;

    // 精度: 約 2^-30 ≈ 1e-9 レベル
    for _ in 0..30 {
        let mid = (lo + hi) * 0.5;
        let x = bezier_component(mid, spline.x1, spline.x2);
        if x < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let u = (lo + hi) * 0.5;
    bezier_component(u, spline.y1, spline.y2)
}

/// ベジェ曲線の 1 成分を計算する。
///
/// P0=0, P1=c1, P2=c2, P3=1 とした三次ベジェの値:
/// B(u) = 3(1-u)^2 * u * c1 + 3(1-u) * u^2 * c2 + u^3
#[inline]
fn bezier_component(u: f64, c1: f64, c2: f64) -> f64 {
    let inv = 1.0 - u;
    3.0 * inv * inv * u * c1 + 3.0 * inv * u * u * c2 + u * u * u
}

/// 2 つの SmilValue を線形補間する。
///
/// 型が一致しない場合は v0 をそのまま返す。
fn interpolate_smil_value(v0: &SmilValue, v1: &SmilValue, t: f64) -> SmilValue {
    match (v0, v1) {
        (SmilValue::Number(a), SmilValue::Number(b)) => {
            SmilValue::Number(lerp_f64(*a, *b, t))
        }
        (SmilValue::NumberPair(a1, a2), SmilValue::NumberPair(b1, b2)) => {
            SmilValue::NumberPair(lerp_f64(*a1, *b1, t), lerp_f64(*a2, *b2, t))
        }
        (SmilValue::NumberTriple(a1, a2, a3), SmilValue::NumberTriple(b1, b2, b3)) => {
            SmilValue::NumberTriple(
                lerp_f64(*a1, *b1, t),
                lerp_f64(*a2, *b2, t),
                lerp_f64(*a3, *b3, t),
            )
        }
        (SmilValue::Color(c0), SmilValue::Color(c1)) => SmilValue::Color(Color {
            r: lerp_u8(c0.r, c1.r, t),
            g: lerp_u8(c0.g, c1.g, t),
            b: lerp_u8(c0.b, c1.b, t),
            a: lerp_u8(c0.a, c1.a, t),
        }),
        // 型不一致のフォールバック
        _ => v0.clone(),
    }
}

/// f64 の線形補間
#[inline]
fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// u8 の線形補間 (四捨五入)
#[inline]
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u8
}

// ============================================
// テスト
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用のデフォルト SmilAnimation を生成するヘルパー
    fn make_anim(values: Vec<SmilValue>) -> SmilAnimation {
        SmilAnimation {
            target_id: "test".to_string(),
            kind: SmilKind::Animate,
            attribute: "opacity".to_string(),
            values,
            key_times: None,
            key_splines: None,
            timing: SmilTiming {
                begin: 0.0,
                duration: 1.0,
                repeat_count: RepeatCount::Definite(1.0),
            },
            calc_mode: CalcMode::Linear,
            additive: Additive::Replace,
            fill_mode: FillMode::Remove,
            transform_type: None,
        }
    }

    // --- begin 前は None ---
    #[test]
    fn before_begin_returns_none() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(1.0)]);
        anim.timing.begin = 2.0;
        assert_eq!(evaluate(&anim, 1.0), None);
    }

    // --- Linear 補間: Number ---
    #[test]
    fn linear_interpolation_number() {
        let anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(10.0)]);
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 5.0).abs() < 1e-9, "expected 5.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- Linear 補間: NumberPair ---
    #[test]
    fn linear_interpolation_number_pair() {
        let anim = make_anim(vec![
            SmilValue::NumberPair(0.0, 100.0),
            SmilValue::NumberPair(10.0, 200.0),
        ]);
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::NumberPair(a, b) => {
                assert!((a - 5.0).abs() < 1e-9, "expected 5.0, got {a}");
                assert!((b - 150.0).abs() < 1e-9, "expected 150.0, got {b}");
            }
            _ => panic!("expected NumberPair"),
        }
    }

    // --- Linear 補間: Color ---
    #[test]
    fn linear_interpolation_color() {
        let anim = make_anim(vec![
            SmilValue::Color(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
            SmilValue::Color(Color {
                r: 255,
                g: 100,
                b: 50,
                a: 255,
            }),
        ]);
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Color(c) => {
                assert_eq!(c.r, 128, "expected r=128, got {}", c.r);
                assert_eq!(c.g, 50, "expected g=50, got {}", c.g);
                assert_eq!(c.b, 25, "expected b=25, got {}", c.b);
                assert_eq!(c.a, 255);
            }
            _ => panic!("expected Color"),
        }
    }

    // --- Discrete モード ---
    #[test]
    fn discrete_mode_returns_start_value() {
        let mut anim = make_anim(vec![SmilValue::Number(10.0), SmilValue::Number(20.0)]);
        anim.calc_mode = CalcMode::Discrete;
        // 50% 地点でもセグメント開始値 (10.0) を返す
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 10.0).abs() < 1e-9, "expected 10.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- Spline モード ---
    #[test]
    fn spline_mode_with_ease_in_out() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(100.0)]);
        anim.calc_mode = CalcMode::Spline;
        // ease-in-out に近いスプライン
        anim.key_splines = Some(vec![CubicBezier1D {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        }]);

        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Number(v) => {
                // ease-in-out の中間点は約 50 だが、正確な値は曲線次第
                // 少なくとも 0 と 100 の間であること
                assert!(v > 0.0 && v < 100.0, "expected 0 < v < 100, got {v}");
            }
            _ => panic!("expected Number"),
        }

        // 端点の確認: t=0 付近は 0 に近い
        let result_start = evaluate(&anim, 0.01).unwrap();
        match result_start {
            SmilValue::Number(v) => assert!(v < 5.0, "expected near 0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- Freeze ---
    #[test]
    fn freeze_at_end() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(100.0)]);
        anim.fill_mode = FillMode::Freeze;

        // アニメーション終了後も最後の値を返す
        let result = evaluate(&anim, 2.0).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 100.0).abs() < 1e-9, "expected 100.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- Remove ---
    #[test]
    fn remove_at_end_returns_none() {
        let anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(100.0)]);
        // fill_mode はデフォルトで Remove
        assert_eq!(evaluate(&anim, 2.0), None);
    }

    // --- Repeat count ---
    #[test]
    fn repeat_count() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(10.0)]);
        anim.timing.repeat_count = RepeatCount::Definite(3.0);
        anim.fill_mode = FillMode::Freeze;

        // duration=1.0, repeat_count=3.0 なので active_duration=3.0
        // elapsed=1.5 → local_time=1.5 → repeat_time=0.5 → progress=0.5 → 値は 5.0
        let result = evaluate(&anim, 1.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 5.0).abs() < 1e-9, "expected 5.0, got {v}"),
            _ => panic!("expected Number"),
        }

        // elapsed=3.0 → active_duration 終了 → Freeze で最後の値
        let result_end = evaluate(&anim, 3.0).unwrap();
        match result_end {
            SmilValue::Number(v) => assert!((v - 10.0).abs() < 1e-9, "expected 10.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- Indefinite repeat ---
    #[test]
    fn indefinite_repeat() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(10.0)]);
        anim.timing.repeat_count = RepeatCount::Indefinite;

        // 100.5 秒後でもアニメーション中 → repeat_time=0.5 → progress=0.5
        let result = evaluate(&anim, 100.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 5.0).abs() < 1e-9, "expected 5.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- keyTimes 不均等配置 ---
    #[test]
    fn key_times_non_uniform() {
        let mut anim = make_anim(vec![
            SmilValue::Number(0.0),
            SmilValue::Number(100.0),
            SmilValue::Number(200.0),
        ]);
        // 最初の 80% の時間で 0→100、残り 20% で 100→200
        anim.key_times = Some(vec![0.0, 0.8, 1.0]);

        // progress=0.4 → 最初のセグメント (0.0..0.8)
        // セグメント内進行度 = 0.4/0.8 = 0.5
        // 値 = 0 + (100-0)*0.5 = 50
        let result = evaluate(&anim, 0.4).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 50.0).abs() < 1e-9, "expected 50.0, got {v}"),
            _ => panic!("expected Number"),
        }

        // progress=0.9 → 2 番目のセグメント (0.8..1.0)
        // セグメント内進行度 = (0.9-0.8)/(1.0-0.8) = 0.5
        // 値 = 100 + (200-100)*0.5 = 150
        let result2 = evaluate(&anim, 0.9).unwrap();
        match result2 {
            SmilValue::Number(v) => assert!((v - 150.0).abs() < 1e-9, "expected 150.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- エッジケース: 空の values ---
    #[test]
    fn empty_values_returns_none() {
        let anim = make_anim(vec![]);
        assert_eq!(evaluate(&anim, 0.5), None);
    }

    // --- エッジケース: 値 1 つ ---
    #[test]
    fn single_value_returns_that_value() {
        let anim = make_anim(vec![SmilValue::Number(42.0)]);
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 42.0).abs() < 1e-9, "expected 42.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- エッジケース: 値 1 つで begin 前 ---
    #[test]
    fn single_value_before_begin_returns_none() {
        let mut anim = make_anim(vec![SmilValue::Number(42.0)]);
        anim.timing.begin = 5.0;
        assert_eq!(evaluate(&anim, 1.0), None);
    }

    // --- 3 つ以上の値での Linear 補間 ---
    #[test]
    fn linear_interpolation_three_values() {
        let anim = make_anim(vec![
            SmilValue::Number(0.0),
            SmilValue::Number(50.0),
            SmilValue::Number(100.0),
        ]);
        // duration=1.0, 等間隔なので 0.25 は最初のセグメントの 50% 地点
        // raw = 0.25 * 2 = 0.5, idx=0, seg_progress=0.5
        // 値 = 0 + (50-0)*0.5 = 25
        let result = evaluate(&anim, 0.25).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 25.0).abs() < 1e-9, "expected 25.0, got {v}"),
            _ => panic!("expected Number"),
        }

        // 0.75 は 2 番目のセグメントの 50% 地点
        // raw = 0.75 * 2 = 1.5, idx=1, seg_progress=0.5
        // 値 = 50 + (100-50)*0.5 = 75
        let result2 = evaluate(&anim, 0.75).unwrap();
        match result2 {
            SmilValue::Number(v) => assert!((v - 75.0).abs() < 1e-9, "expected 75.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    // --- NumberTriple の補間 ---
    #[test]
    fn linear_interpolation_number_triple() {
        let anim = make_anim(vec![
            SmilValue::NumberTriple(0.0, 0.0, 0.0),
            SmilValue::NumberTriple(10.0, 20.0, 30.0),
        ]);
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::NumberTriple(a, b, c) => {
                assert!((a - 5.0).abs() < 1e-9);
                assert!((b - 10.0).abs() < 1e-9);
                assert!((c - 15.0).abs() < 1e-9);
            }
            _ => panic!("expected NumberTriple"),
        }
    }

    // --- duration == 0 のエッジケース ---
    #[test]
    fn zero_duration_freeze() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(42.0)]);
        anim.timing.duration = 0.0;
        anim.fill_mode = FillMode::Freeze;
        let result = evaluate(&anim, 0.5).unwrap();
        match result {
            SmilValue::Number(v) => assert!((v - 42.0).abs() < 1e-9, "expected 42.0, got {v}"),
            _ => panic!("expected Number"),
        }
    }

    #[test]
    fn zero_duration_remove() {
        let mut anim = make_anim(vec![SmilValue::Number(0.0), SmilValue::Number(42.0)]);
        anim.timing.duration = 0.0;
        anim.fill_mode = FillMode::Remove;
        assert_eq!(evaluate(&anim, 0.5), None);
    }

    // --- ヘルパー関数のテスト ---

    #[test]
    fn find_segment_basic() {
        let kt = vec![0.0, 0.25, 0.75, 1.0];
        let (idx, prog) = find_segment(&kt, 0.5);
        assert_eq!(idx, 1);
        assert!((prog - 0.5).abs() < 1e-9); // (0.5-0.25)/(0.75-0.25) = 0.5
    }

    #[test]
    fn find_segment_at_boundary() {
        let kt = vec![0.0, 0.5, 1.0];
        let (idx, prog) = find_segment(&kt, 0.5);
        assert_eq!(idx, 1);
        assert!((prog - 0.0).abs() < 1e-9);
    }

    #[test]
    fn bezier_linear() {
        // 制御点が線形: (0.5, 0.5) → 恒等関数に近い
        let spline = CubicBezier1D {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let result = solve_cubic_bezier_1d(&spline, 0.5);
        assert!(
            (result - 0.5).abs() < 0.01,
            "expected ~0.5, got {result}"
        );
    }

    #[test]
    fn bezier_endpoints() {
        let spline = CubicBezier1D {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        };
        assert!((solve_cubic_bezier_1d(&spline, 0.0)).abs() < 1e-6);
        assert!((solve_cubic_bezier_1d(&spline, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_u8_midpoint() {
        assert_eq!(lerp_u8(0, 255, 0.5), 128);
        assert_eq!(lerp_u8(0, 100, 0.5), 50);
    }
}

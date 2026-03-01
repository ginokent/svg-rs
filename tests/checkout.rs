//! checkout.svg を使用した統合テスト。
//! パース結果の検証、SMIL 評価、パスフラッテニングを包括的にテストする。

/// checkout.svg のバイト列を返すヘルパー。
fn checkout_svg() -> &'static [u8] {
    include_bytes!("fixtures/checkout.svg")
}

// ============================================
// パース結果の検証
// ============================================

#[test]
fn test_checkout_parse_viewbox() {
    let doc = svg::parse(checkout_svg()).unwrap();
    assert_eq!(doc.view_box.x, 0.0);
    assert_eq!(doc.view_box.y, 0.0);
    assert_eq!(doc.view_box.width, 400.0);
    assert_eq!(doc.view_box.height, 400.0);
}

#[test]
fn test_checkout_has_animations() {
    let doc = svg::parse(checkout_svg()).unwrap();
    // checkout.svg は 18 個の SMIL アニメーション要素を持つ
    assert!(
        doc.animations.len() >= 15,
        "Expected at least 15 animations, got {}",
        doc.animations.len()
    );
}

#[test]
fn test_checkout_has_paths() {
    let doc = svg::parse(checkout_svg()).unwrap();
    // シーンツリーにパスが含まれていることを確認
    let path_count = count_paths(&doc.root);
    assert!(
        path_count >= 10,
        "Expected at least 10 paths, got {}",
        path_count
    );
}

/// SvgGroup 内のパス数を再帰的にカウントする。
fn count_paths(group: &svg::SvgGroup) -> usize {
    let mut count = 0;
    for child in &group.children {
        match child {
            svg::SvgNode::Path(_) => count += 1,
            svg::SvgNode::Group(g) => count += count_paths(g),
        }
    }
    count
}

// ============================================
// SMIL 評価テスト
// ============================================

#[test]
fn test_checkout_smil_evaluate_at_zero() {
    let doc = svg::parse(checkout_svg()).unwrap();
    // t=0 でアニメーションを評価
    for anim in &doc.animations {
        // begin=0 のアニメーションは t=0 で Some を返すはず
        if anim.timing.begin == 0.0 && anim.values.len() >= 2 {
            let val = svg::evaluate(anim, 0.0);
            assert!(
                val.is_some(),
                "Animation targeting '{}' attr='{}' should have value at t=0",
                anim.target_id, anim.attribute
            );
        }
    }
}

#[test]
fn test_checkout_smil_evaluate_at_midpoint() {
    let doc = svg::parse(checkout_svg()).unwrap();
    // t=1.5 (3s アニメーションの中間) で評価
    for anim in &doc.animations {
        if anim.timing.begin == 0.0 && anim.timing.duration > 0.0 {
            let val = svg::evaluate(anim, 1.5);
            assert!(
                val.is_some(),
                "Animation targeting '{}' attr='{}' should have value at t=1.5",
                anim.target_id, anim.attribute
            );
        }
    }
}

// ============================================
// パスフラッテニングテスト
// ============================================

#[test]
fn test_checkout_all_paths_flattenable() {
    let doc = svg::parse(checkout_svg()).unwrap();
    // 全パスがフラッテニング可能であることを確認 (パニックしないこと)
    check_flattenable(&doc.root);
}

/// SvgGroup 内の全パスをフラッテニングし、結果が空でないことを確認する。
fn check_flattenable(group: &svg::SvgGroup) {
    for child in &group.children {
        match child {
            svg::SvgNode::Path(path) => {
                let polylines = svg::flatten(&path.segments, 0.25);
                assert!(
                    !polylines.is_empty(),
                    "Path {:?} should produce at least one polyline",
                    path.id
                );
                for polyline in &polylines {
                    assert!(
                        polyline.len() >= 2,
                        "Polyline from path {:?} should have at least 2 points",
                        path.id
                    );
                }
            }
            svg::SvgNode::Group(g) => check_flattenable(g),
        }
    }
}

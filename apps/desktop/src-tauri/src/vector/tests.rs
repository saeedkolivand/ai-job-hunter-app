use super::cosine;

#[test]
fn identical_vectors_score_one() {
    let a = [1.0, 2.0, 3.0];
    assert!((cosine(&a, &a) - 1.0).abs() < 1e-9);
}

#[test]
fn orthogonal_vectors_score_zero() {
    assert!((cosine(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-9);
}

#[test]
fn degenerate_inputs_score_zero() {
    assert_eq!(cosine(&[], &[]), 0.0);
    assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0); // length mismatch
    assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0); // zero magnitude
}

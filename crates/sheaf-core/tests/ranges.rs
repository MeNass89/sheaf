use sheaf_core::{ranges::parse_ranges, OpdfError};
#[test]
fn parses_mixed_spec_in_order() {
    assert_eq!(parse_ranges("3, 1-2 ,5", 5).unwrap(), vec![3..=3, 1..=2, 5..=5]);
}
#[test]
fn open_end_means_last_page() {
    assert_eq!(parse_ranges("2-", 4).unwrap(), vec![2..=4]);
}
#[test]
fn range_past_end_is_rejected() {
    let e = parse_ranges("1-40", 3).unwrap_err();
    assert!(matches!(e, OpdfError::InvalidRange { pages: 3, .. }));
}
#[test]
fn reversed_and_zero_are_rejected() {
    assert!(parse_ranges("3-1", 5).is_err());
    assert!(parse_ranges("0", 5).is_err());
    assert!(parse_ranges("", 5).is_err());
}
#[test]
fn open_start_is_rejected() {
    assert!(parse_ranges("-3", 5).is_err());
}

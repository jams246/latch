use stacksafe::stacksafe;

#[test]
fn recursive_function_returns_its_result() {
    #[stacksafe]
    fn sum(value: usize) -> usize {
        if value == 0 {
            0
        } else {
            value + sum(value - 1)
        }
    }

    assert_eq!(sum(100), 5050);
}

#[test]
fn explicit_crate_path_supports_impl_trait_return() {
    use stacksafe as renamed;

    #[stacksafe(crate = renamed)]
    fn values() -> impl Iterator<Item = u8> {
        0..3
    }

    assert_eq!(values().collect::<Vec<_>>(), vec![0, 1, 2]);
}

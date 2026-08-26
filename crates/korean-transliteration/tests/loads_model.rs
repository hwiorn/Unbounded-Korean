#[test]
fn eng_model_loads_without_panicking() {
    let _ = korean_transliteration::transliterate("eng", "hello");
}

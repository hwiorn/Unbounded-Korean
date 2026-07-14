use criterion::{Criterion, criterion_group, criterion_main};
use g2pk::G2p;
use hangulize_rs::Hangulizer;
use korean_phonemizer::{
    PhonemizerMode, PhonemizerOptions, korean_to_ipa, phonemize_ko_with_options,
};

fn bench_g2p(c: &mut Criterion) {
    let g2p = G2p::new().unwrap();
    let text = "나의 친구가 mp3 file 3개를 다운받고 있다. 학교에 갔다 와서 밥을 먹었다.";

    c.bench_function("g2p_convert_sentence", |b| {
        b.iter(|| g2p.convert(text).unwrap())
    });
}

fn bench_phonemizer(c: &mut Criterion) {
    let spoken = "나의 친구가 엠피쓰리 파일 세개를 다운받꼬 읻따";
    let opts = PhonemizerOptions::default();

    c.bench_function("korean_to_ipa_table_spoken", |b| {
        b.iter(|| korean_to_ipa(spoken, &opts))
    });

    c.bench_function("phonemize_ko_table_pipeline", |b| {
        b.iter(|| phonemize_ko_with_options(spoken, &opts).unwrap())
    });
}

fn bench_epitran(c: &mut Criterion) {
    let opts = PhonemizerOptions {
        mode: PhonemizerMode::Epitran,
        ..PhonemizerOptions::default()
    };

    c.bench_function("phonemize_ko_epitran_pipeline", |b| {
        b.iter(|| phonemize_ko_with_options("한글 학교", &opts).unwrap())
    });
}

fn bench_hangulize(c: &mut Criterion) {
    let ita = Hangulizer::new("ita").unwrap();
    let rus = Hangulizer::new("rus").unwrap();

    c.bench_function("hangulize_ita", |b| {
        b.iter(|| ita.hangulize("Cappuccino").unwrap())
    });

    c.bench_function("hangulize_rus_cyrillic", |b| {
        b.iter(|| rus.hangulize("Владивосток").unwrap())
    });
}

criterion_group!(
    benches,
    bench_g2p,
    bench_phonemizer,
    bench_epitran,
    bench_hangulize
);
criterion_main!(benches);

from collect_hsl_seed_data import parse_hsl_tests


def test_parses_eng_hsl_test_block(tmp_path):
    hsl = tmp_path / "eng.hsl"
    hsl.write_text(
        'lang:\n    id = "eng"\n'
        "test:\n"
        '    "SKT"   -> "에스케이티"\n'
        '    "hello" -> "헬로"\n'
    )
    pairs = parse_hsl_tests(hsl.read_text())
    assert pairs == [("SKT", "에스케이티"), ("hello", "헬로")]


def test_ignores_lines_outside_test_block(tmp_path):
    hsl = tmp_path / "spa.hsl"
    hsl.write_text(
        'lang:\n    id = "spa"\n'
        "rewrite:\n"
        '    "a" -> "a" # not a test pair\n'
        "test:\n"
        '    "hola" -> "올라"\n'
    )
    pairs = parse_hsl_tests(hsl.read_text())
    assert pairs == [("hola", "올라")]


def test_strips_trailing_comments(tmp_path):
    hsl = tmp_path / "ita.hsl"
    hsl.write_text('test:\n    "ciao" -> "차오" # greeting\n')
    pairs = parse_hsl_tests(hsl.read_text())
    assert pairs == [("ciao", "차오")]

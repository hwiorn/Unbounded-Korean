use g2pk::{G2p, G2pConfig, G2pOptions, ResourceConfig};
use korean_phonemizer::{IpaStyle, PhonemizerMode, PhonemizerOptions};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

#[allow(unsafe_op_in_unsafe_fn)]
#[pyclass(name = "G2p")]
struct PyG2p {
    inner: G2p,
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pymethods]
impl PyG2p {
    #[new]
    #[pyo3(signature = (table_csv=None, idioms_txt=None))]
    fn new(table_csv: Option<String>, idioms_txt: Option<String>) -> PyResult<Self> {
        let inner = G2p::with_config(G2pConfig {
            resources: ResourceConfig {
                table_csv,
                idioms_txt,
            },
        })
        .map_err(to_runtime_error)?;
        Ok(Self { inner })
    }

    #[pyo3(signature = (text, descriptive=false, group_vowels=false, to_syl=true))]
    fn __call__(
        &self,
        text: &str,
        descriptive: bool,
        group_vowels: bool,
        to_syl: bool,
    ) -> PyResult<String> {
        self.convert(text, descriptive, group_vowels, to_syl)
    }

    #[pyo3(signature = (text, descriptive=false, group_vowels=false, to_syl=true))]
    fn convert(
        &self,
        text: &str,
        descriptive: bool,
        group_vowels: bool,
        to_syl: bool,
    ) -> PyResult<String> {
        self.inner
            .convert_with_options(
                text,
                &G2pOptions {
                    descriptive,
                    group_vowels,
                    to_syl,
                },
            )
            .map_err(to_runtime_error)
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pyclass(name = "KoreanPhonemizer")]
struct PyKoreanPhonemizer {
    options: PhonemizerOptions,
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pymethods]
impl PyKoreanPhonemizer {
    #[new]
    #[pyo3(signature = (mode="kog2p_table", epitran_compat=true, colloquial=false, ipa_style="combining"))]
    fn new(mode: &str, epitran_compat: bool, colloquial: bool, ipa_style: &str) -> PyResult<Self> {
        Ok(Self {
            options: parse_phonemizer_options(mode, epitran_compat, colloquial, ipa_style)?,
        })
    }

    fn phonemize(&self, text: &str) -> PyResult<(String, String)> {
        let out = korean_phonemizer::phonemize_ko_with_options(text, &self.options)
            .map_err(to_runtime_error)?;
        Ok((out.spoken, out.ipa))
    }

    fn __call__(&self, text: &str) -> PyResult<(String, String)> {
        self.phonemize(text)
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pyclass(name = "Hangulizer")]
struct PyHangulizer {
    inner: hangulize_rs::Hangulizer,
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pymethods]
impl PyHangulizer {
    #[new]
    fn new(lang: &str) -> PyResult<Self> {
        let inner = hangulize_rs::Hangulizer::new(lang).map_err(to_hangulize_error)?;
        Ok(Self { inner })
    }

    #[getter]
    fn lang(&self) -> String {
        self.inner.lang().to_string()
    }

    fn hangulize(&self, word: &str) -> PyResult<String> {
        self.inner.hangulize(word).map_err(to_hangulize_error)
    }

    fn __call__(&self, word: &str) -> PyResult<String> {
        self.hangulize(word)
    }
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
#[pyo3(signature = (text, descriptive=false, group_vowels=false, to_syl=true))]
fn g2p(text: &str, descriptive: bool, group_vowels: bool, to_syl: bool) -> PyResult<String> {
    let g2p = G2p::new().map_err(to_runtime_error)?;
    g2p.convert_with_options(
        text,
        &G2pOptions {
            descriptive,
            group_vowels,
            to_syl,
        },
    )
    .map_err(to_runtime_error)
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
#[pyo3(signature = (text, mode="kog2p_table", epitran_compat=true, colloquial=false, ipa_style="combining"))]
fn phonemize_ko(
    text: &str,
    mode: &str,
    epitran_compat: bool,
    colloquial: bool,
    ipa_style: &str,
) -> PyResult<(String, String)> {
    let options = parse_phonemizer_options(mode, epitran_compat, colloquial, ipa_style)?;
    let out =
        korean_phonemizer::phonemize_ko_with_options(text, &options).map_err(to_runtime_error)?;
    Ok((out.spoken, out.ipa))
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
fn hangulize(lang: &str, word: &str) -> PyResult<String> {
    hangulize_rs::hangulize(lang, word).map_err(to_hangulize_error)
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
fn list_langs() -> Vec<String> {
    hangulize_rs::list_langs()
}

fn parse_phonemizer_options(
    mode: &str,
    epitran_compat: bool,
    colloquial: bool,
    ipa_style: &str,
) -> PyResult<PhonemizerOptions> {
    let mode = PhonemizerMode::parse(mode).map_err(|err| PyValueError::new_err(err.to_string()))?;
    let ipa_style =
        IpaStyle::parse(ipa_style).map_err(|err| PyValueError::new_err(err.to_string()))?;
    Ok(PhonemizerOptions {
        mode,
        epitran_compat,
        colloquial,
        ipa_style,
    })
}

fn to_hangulize_error(err: hangulize_rs::Error) -> PyErr {
    match err {
        hangulize_rs::Error::SpecNotFound(message) => PyValueError::new_err(message),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

fn to_runtime_error<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

#[pymodule]
fn unbounded_korean(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyG2p>()?;
    m.add_class::<PyKoreanPhonemizer>()?;
    m.add_class::<PyHangulizer>()?;
    m.add_function(wrap_pyfunction!(g2p, m)?)?;
    m.add_function(wrap_pyfunction!(phonemize_ko, m)?)?;
    m.add_function(wrap_pyfunction!(hangulize, m)?)?;
    m.add_function(wrap_pyfunction!(list_langs, m)?)?;
    Ok(())
}

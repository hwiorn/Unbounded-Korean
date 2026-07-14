use ::korean_phonemizer as core;
use core::{IpaStyle, PhonemizerMode, PhonemizerOptions};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

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
            options: parse_options(mode, epitran_compat, colloquial, ipa_style)?,
        })
    }

    fn phonemize(&self, text: &str) -> PyResult<(String, String)> {
        let out = core::phonemize_ko_with_options(text, &self.options).map_err(to_runtime_error)?;
        Ok((out.spoken, out.ipa))
    }

    fn __call__(&self, text: &str) -> PyResult<(String, String)> {
        self.phonemize(text)
    }
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
    let options = parse_options(mode, epitran_compat, colloquial, ipa_style)?;
    let out = core::phonemize_ko_with_options(text, &options).map_err(to_runtime_error)?;
    Ok((out.spoken, out.ipa))
}

fn parse_options(
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

fn to_runtime_error<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

#[pymodule]
fn korean_phonemizer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKoreanPhonemizer>()?;
    m.add_function(wrap_pyfunction!(phonemize_ko, m)?)?;
    Ok(())
}

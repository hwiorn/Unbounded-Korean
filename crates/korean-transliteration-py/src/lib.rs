use ::korean_transliteration as core;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
fn transliterate(lang: &str, word: &str) -> PyResult<String> {
    core::transliterate(lang, word).map_err(to_py_error)
}

fn to_py_error(err: core::Error) -> PyErr {
    match err {
        core::Error::ModelNotFound(message) => PyValueError::new_err(message),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

#[pymodule]
fn korean_transliteration(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(transliterate, m)?)?;
    Ok(())
}

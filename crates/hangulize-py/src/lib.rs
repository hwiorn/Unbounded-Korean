use ::hangulize_rs as core;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

#[allow(unsafe_op_in_unsafe_fn)]
#[pyclass(name = "Hangulizer")]
struct PyHangulizer {
    inner: core::Hangulizer,
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pymethods]
impl PyHangulizer {
    #[new]
    fn new(lang: &str) -> PyResult<Self> {
        let inner = core::Hangulizer::new(lang).map_err(to_py_error)?;
        Ok(Self { inner })
    }

    #[getter]
    fn lang(&self) -> String {
        self.inner.lang().to_string()
    }

    fn hangulize(&self, word: &str) -> PyResult<String> {
        self.inner.hangulize(word).map_err(to_py_error)
    }

    fn __call__(&self, word: &str) -> PyResult<String> {
        self.hangulize(word)
    }
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
fn list_langs() -> Vec<String> {
    core::list_langs()
}

#[pyfunction]
#[allow(unsafe_op_in_unsafe_fn)]
fn hangulize(lang: &str, word: &str) -> PyResult<String> {
    core::hangulize(lang, word).map_err(to_py_error)
}

fn to_py_error(err: core::Error) -> PyErr {
    match err {
        core::Error::SpecNotFound(message) => PyValueError::new_err(message),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

#[pymodule]
fn hangulize_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHangulizer>()?;
    m.add_function(wrap_pyfunction!(list_langs, m)?)?;
    m.add_function(wrap_pyfunction!(hangulize, m)?)?;
    Ok(())
}

use g2pk::{G2p, G2pConfig, G2pOptions, ResourceConfig};
use pyo3::exceptions::PyRuntimeError;
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

    #[pyo3(signature = (text, descriptive=false, group_vowels=false, to_syl=true))]
    fn convert(
        &self,
        text: &str,
        descriptive: bool,
        group_vowels: bool,
        to_syl: bool,
    ) -> PyResult<String> {
        self.__call__(text, descriptive, group_vowels, to_syl)
    }
}

fn to_runtime_error<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

#[pymodule]
fn g2pk_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyG2p>()?;
    Ok(())
}

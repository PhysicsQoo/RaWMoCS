use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rawmocs_mpfr::{CancellationToken, MpfrBinary, Precision, SolveDiagnostics, SolveFailure, SolveOptions, SolveReport, SolverMode};
use std::path::PathBuf;
use std::sync::Arc;

create_exception!(_native, NativeSolveError, PyException);

#[pyclass(frozen)]
struct NativeCancellationToken {
    inner: Arc<CancellationToken>,
}

#[pymethods]
impl NativeCancellationToken {
    #[new]
    fn new() -> Self {
        Self { inner: Arc::new(CancellationToken::new()) }
    }

    fn cancel(&self) {
        self.inner.cancel();
    }

    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

fn options(digits: u32, mode: &str, timeout_ms: Option<u64>, large_general: bool) -> Result<SolveOptions, SolveFailure> {
    let precision = Precision::from_digits(digits)
        .ok_or_else(|| SolveFailure::from_reason("INPUT: unsupported digits".into()))?;
    let mode = SolverMode::from_name(mode)
        .ok_or_else(|| SolveFailure::from_reason("INPUT: invalid solver".into()))?;
    Ok(SolveOptions { precision, mode, timeout_ms, large_general })
}

fn binary<'py>(py: Python<'py>, value: MpfrBinary) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("significand", value.significand)?;
    d.set_item("exponent2", value.exponent2)?;
    d.set_item("precision_bits", value.precision_bits)?;
    d.set_item("negative_zero", value.negative_zero)?;
    Ok(d)
}

fn diagnostics<'py>(py: Python<'py>, value: &SolveDiagnostics) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    macro_rules! stage {
        ($name:ident, $($field:ident),+) => {
            if let Some(v) = &value.$name {
                let item = PyDict::new(py);
                $(item.set_item(stringify!($field), v.$field)?;)+
                d.set_item(stringify!($name), item)?;
            } else {
                d.set_item(stringify!($name), py.None())?;
            }
        }
    }
    if let Some(v) = &value.fallback {
        let item = PyDict::new(py);
        item.set_item("reason", &v.reason)?;
        item.set_item("route", v.route.as_str())?;
        d.set_item("fallback", item)?;
    } else {
        d.set_item("fallback", py.None())?;
    }
    stage!(streaming_source, variables, clauses, literals, parse_ns);
    stage!(general, max_frames);
    stage!(separator, checks, overrides);
    stage!(frontier_stop, steps, remaining);
    stage!(prejoin, residual, cuts, leaves, width, work, peak, joins, plan_ns, local_ns, choose_ns, restrict_ns, execute_ns);
    stage!(binary, leaves, ops, visits, join_visits);
    Ok(d)
}

fn finish(py: Python<'_>, outcome: Result<SolveReport, SolveFailure>) -> PyResult<Py<PyDict>> {
    match outcome {
        Ok(report) => {
            let d = PyDict::new(py);
            d.set_item("sat", report.sat)?;
            d.set_item("real", &report.real)?;
            d.set_item("imag", &report.imag)?;
            d.set_item("digits", report.precision.digits())?;
            d.set_item("precision_bits", report.precision.bits())?;
            d.set_item("route", report.route.as_str())?;
            d.set_item("solve_ns", report.solve_ns)?;
            d.set_item("real_binary", binary(py, report.real_mpfr().binary())?)?;
            d.set_item("imag_binary", binary(py, report.imag_mpfr().binary())?)?;
            d.set_item("diagnostics", diagnostics(py, &report.diagnostics)?)?;
            Ok(d.unbind())
        }
        Err(error) => {
            let diag = diagnostics(py, &error.diagnostics)?.unbind();
            Err(NativeSolveError::new_err((error.kind.as_str(), error.reason().to_owned(), diag)))
        }
    }
}

#[pyfunction]
#[pyo3(signature = (cnf, digits, mode, timeout_ms, large_general, token=None))]
fn solve_text(py: Python<'_>, cnf: String, digits: u32, mode: &str, timeout_ms: Option<u64>, large_general: bool, token: Option<PyRef<'_, NativeCancellationToken>>) -> PyResult<Py<PyDict>> {
    let opt = options(digits, mode, timeout_ms, large_general);
    let token = token.map(|t| Arc::clone(&t.inner));
    let outcome = py.allow_threads(move || {
        let opt = opt?;
        match token {
            Some(t) => rawmocs_mpfr::solve_text_cancellable(&cnf, opt, &t),
            None => rawmocs_mpfr::solve_text(&cnf, opt),
        }
    });
    finish(py, outcome)
}

#[pyfunction]
#[pyo3(signature = (path, digits, mode, timeout_ms, large_general, token=None))]
fn solve_file(py: Python<'_>, path: PathBuf, digits: u32, mode: &str, timeout_ms: Option<u64>, large_general: bool, token: Option<PyRef<'_, NativeCancellationToken>>) -> PyResult<Py<PyDict>> {
    let opt = options(digits, mode, timeout_ms, large_general);
    let token = token.map(|t| Arc::clone(&t.inner));
    let outcome = py.allow_threads(move || {
        let opt = opt?;
        match token {
            Some(t) => rawmocs_mpfr::solve_file_cancellable(path.as_os_str(), opt, &t),
            None => rawmocs_mpfr::solve_file(path.as_os_str(), opt),
        }
    });
    finish(py, outcome)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeCancellationToken>()?;
    m.add("NativeSolveError", m.py().get_type::<NativeSolveError>())?;
    m.add_function(wrap_pyfunction!(solve_text, m)?)?;
    m.add_function(wrap_pyfunction!(solve_file, m)?)?;
    Ok(())
}

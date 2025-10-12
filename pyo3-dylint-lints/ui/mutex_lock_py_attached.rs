#![allow(unused)]

use pyo3::marker::Python;
use pyo3::sync::MutexExt;
use std::sync::Mutex as StdMutex;
use parking_lot::Mutex as PLMutex;

// Should trigger - PyO3 function without explicit Python token argument
#[pyo3::pyfunction]
fn bad_pyfunction() -> pyo3::PyResult<()> {
    let mutex = StdMutex::new(0);
    let _guard = mutex.lock().unwrap();
    Ok(())
}

// Should trigger - PyO3 class method without explicit Python token argument
#[pyo3::pyclass]
struct PyClassWithMutex {
    data: StdMutex<i32>,
}

#[pyo3::pymethods]
impl PyClassWithMutex {
    fn bad_method(&self) -> pyo3::PyResult<i32> {
        let _guard = self.data.lock().unwrap();
        Ok(7)
    }

    fn ok_try_lock(&self) -> pyo3::PyResult<()> {
        if let Ok(_guard) = self.data.try_lock() {
            // work with guard
        }
        Ok(())
    }
}

// Should trigger the lint
fn bad_example_basic_std(_py: Python<'_>, mutex: &StdMutex<i32>) {
    let _guard = mutex.lock().unwrap();
}

fn bad_example_basic_parking_lot(_py: Python<'_>, mutex: &PLMutex<i32>) {
    let _guard = mutex.lock();
}

// Should trigger the lint - mutex in struct
struct MyStruct {
    data: StdMutex<String>,
}

fn bad_example_struct(_py: Python<'_>, s: &MyStruct) {
    let _guard = s.data.lock().unwrap();
}

// Should NOT trigger - no Python token
fn ok_no_python_token(mutex: &StdMutex<i32>) {
    let _guard = mutex.lock().unwrap();
}

// Should NOT trigger - using lock_py_attached
fn ok_using_lock_py_attached(py: Python<'_>, mutex: &StdMutex<i32>) {
    let _guard = mutex.lock_py_attached(py).unwrap();
}

// Should NOT trigger - try_lock is fine
fn ok_using_try_lock(_py: Python<'_>, mutex: &StdMutex<i32>) {
    if let Ok(_guard) = mutex.try_lock() {
        // work with guard
    }
}

fn main() {}

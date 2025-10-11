use pyo3::prelude::*;
use std::sync::Mutex;

// Should trigger the lint
fn bad_example_basic(py: Python<'_>, mutex: &Mutex<i32>) {
    let _guard = mutex.lock().unwrap();
}

// Should trigger the lint - mutex in struct
struct MyStruct {
    data: Mutex<String>,
}

fn bad_example_struct(py: Python<'_>, s: &MyStruct) {
    let _guard = s.data.lock().unwrap();
}

// Should NOT trigger - no Python token
fn ok_no_python_token(mutex: &Mutex<i32>) {
    let _guard = mutex.lock().unwrap();
}

// Should NOT trigger - using lock_py_attached
fn ok_using_lock_py_attached(py: Python<'_>, mutex: &Mutex<i32>) {
    use pyo3::sync::MutexExt;
    let _guard = mutex.lock_py_attached(py).unwrap();
}

// Should NOT trigger - try_lock is fine
fn ok_using_try_lock(py: Python<'_>, mutex: &Mutex<i32>) {
    if let Ok(_guard) = mutex.try_lock() {
        // work with guard
    }
}

fn main() {}

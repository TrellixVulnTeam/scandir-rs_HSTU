use std::time::{Instant, Duration};
use std::thread;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use jwalk::WalkDir;
#[cfg(unix)]
use expanduser::expanduser;

use pyo3::prelude::*;
use pyo3::types::{PyType, PyAny, PyDict};
use pyo3::{Python, wrap_pyfunction, PyContextProtocol};
use pyo3::exceptions::{self, ValueError};

#[pyclass]
#[derive(Debug, Clone)]
pub struct Statistics {
    pub dirs: u32,
    pub files: u32,
    pub slinks: u32,
    pub hlinks: u32,
    pub devices: u32,
    pub pipes: u32,
    pub size: u64,
    pub usage: u64,
    pub errors: Vec<String>,
}

#[pyproto]
impl pyo3::class::PyObjectProtocol for Statistics {
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}

fn rs_count(
    root_path: String,
    skip_hidden: bool,
    metadata: bool,
    metadata_ext: bool,
    max_depth: usize,
    statistics: Arc<Mutex<Statistics>>,
    alive: Option<Arc<AtomicBool>>,
) {
    #[cfg(unix)]
    let root_path = expanduser(root_path).unwrap();
    let mut dirs: u32 = 0;
    let mut files: u32 = 0;
    let mut slinks: u32 = 0;
    let mut hlinks: u32 = 0;
    let mut size: u64 = 0;
    let mut usage: u64 = 0;
    let mut errors: Vec<String> = Vec::new();
    #[cfg(unix)]
    let mut devices: u32 = 0;
    #[cfg(unix)]
    let mut pipes: u32 = 0;
    let mut cnt: u32 = 0;
    for entry in WalkDir::new(root_path)
        .skip_hidden(skip_hidden)
        .sort(false)
        .preload_metadata(metadata)
        .preload_metadata_ext(metadata_ext)
        .max_depth(max_depth)
    {
        match &entry {
            Ok(v) => {
                let file_type = v.file_type_result.as_ref().unwrap();
                if file_type.is_dir() {
                    dirs += 1;
                }
                else if file_type.is_file() {
                    files += 1;
                }
                else if file_type.is_symlink() {
                    slinks += 1;
                }
                if v.metadata_result.is_some() {
                    let metadata_ext = v.ext.as_ref();
                    if metadata_ext.is_some() {
                        let metadata_ext = metadata_ext.unwrap().as_ref().unwrap();
                        if metadata_ext.nlink > 1 {
                            hlinks += 1;
                        }
                        size += metadata_ext.size;
                        #[cfg(unix)]
                        {
                            if metadata_ext.rdev > 0 {
                                devices += 1;
                            }
                            if (metadata_ext.mode & 4096) != 0 {
                                pipes += 1;
                            }
                            usage += metadata_ext.blocks * 512;
                        }
                        #[cfg(windows)]
                        {
                            let mut blocks = metadata_ext.size >> 12;
                            if blocks << 12 < metadata_ext.size {
                                blocks += 1;
                            }
                            usage += blocks << 12;
                        }
                    }
                }
            }
            Err(e) => errors.push(e.to_string())  // TODO: Need to fetch failed path from somewhere
        }
        cnt += 1;
        if cnt >= 1000 {
            let mut stats_locked = statistics.lock().unwrap();
            stats_locked.dirs = dirs;
            stats_locked.files = files;
            stats_locked.slinks = slinks;
            stats_locked.hlinks = hlinks;
            stats_locked.size = size;
            stats_locked.usage = usage;
            if stats_locked.errors.len() < errors.len() {
                stats_locked.errors.extend_from_slice(&errors);
                errors.clear();
            }
            #[cfg(unix)]
            {
                stats_locked.devices = devices;
                stats_locked.pipes = pipes;
            }
            cnt = 0;
        }
        match &alive {
            Some(a) => if !a.load(Ordering::Relaxed) {
                break;
            },
            None => {},
        }
    }
    {
        let mut stats_locked = statistics.lock().unwrap();
        stats_locked.dirs = dirs;
        stats_locked.files = files;
        stats_locked.slinks = slinks;
        stats_locked.hlinks = hlinks;
        stats_locked.size = size;
        stats_locked.usage = usage;
        if stats_locked.errors.len() < errors.len() {
            stats_locked.errors.extend_from_slice(&errors);
            errors.clear();
        }
        #[cfg(unix)]
        {
            stats_locked.devices = devices;
            stats_locked.pipes = pipes;
        }
    }
}

#[pyfunction]
pub fn count(
    py: Python,
    root_path: String,
    skip_hidden: Option<bool>,
    metadata: Option<bool>,
    metadata_ext: Option<bool>,
    max_depth: Option<usize>,
) -> PyResult<Statistics> {
    let statistics = Arc::new(Mutex::new(Statistics { 
        dirs: 0,
        files: 0,
        slinks: 0,
        hlinks: 0,
        devices: 0,
        pipes: 0,
        size: 0,
        usage: 0,
        errors: Vec::new(),
    }));
    let stats_cloned = statistics.clone();
    let rc: std::result::Result<(), std::io::Error> = py.allow_threads(|| {
        rs_count(root_path,
                 skip_hidden.unwrap_or(false),
                 metadata.unwrap_or(false),
                 metadata_ext.unwrap_or(false),
                 max_depth.unwrap_or(::std::usize::MAX),
                 stats_cloned, None);
        Ok(())
    });
    match rc {
        Err(e) => return Err(exceptions::RuntimeError::py_err(e.to_string())),
        _ => ()
    }
    let stats_cloned = statistics.lock().unwrap().clone();
    Ok(stats_cloned.into())
}

#[pyclass]
#[derive(Debug)]
pub struct Count {
    // Options
    root_path: String,
    skip_hidden: bool,
    metadata: bool,
    metadata_ext: bool,
    max_depth: usize,
    // Results
    statistics: Arc<Mutex<Statistics>>,
    // Internal
    thr: Option<thread::JoinHandle<()>>,
    alive: Option<Weak<AtomicBool>>,
    has_results: bool,
    start_time: Instant,
    duration: Duration,
}

impl Count {
    fn rs_init(&self) {
        let mut stats_locked = self.statistics.lock().unwrap();
        stats_locked.dirs = 0;
        stats_locked.files = 0;
        stats_locked.slinks = 0;
        stats_locked.hlinks = 0;
        stats_locked.size = 0;
        stats_locked.usage = 0;
        stats_locked.errors.clear();
        #[cfg(unix)]
        {
            stats_locked.devices = 0;
            stats_locked.pipes = 0;
        }
    }

    fn rs_start(&mut self) -> bool {
        if self.thr.is_some() {
            return false
        }
        self.start_time = Instant::now();
        if self.has_results {
            self.rs_init();
        }
        let root_path = String::from(&self.root_path);
        let skip_hidden = self.skip_hidden;
        let metadata = self.metadata;
        let metadata_ext = self.metadata_ext;
        let max_depth = self.max_depth;
        let statistics = self.statistics.clone();
        let alive = Arc::new(AtomicBool::new(true));
        self.alive = Some(Arc::downgrade(&alive));
        self.thr = Some(thread::spawn(move || {
            rs_count(root_path,
                     skip_hidden, metadata, metadata_ext, max_depth,
                     statistics, Some(alive))
        }));
        true
    }

    fn rs_stop(&mut self) -> bool {
        match &self.alive {
            Some(alive) => match alive.upgrade() {
                Some(alive) => (*alive).store(false, Ordering::Relaxed),
                None => return false,
            },
            None => {},
        }
        self.thr.take().map(thread::JoinHandle::join);
        self.duration = self.start_time.elapsed();
        self.has_results = true;
        true
    }
}

#[pymethods]
impl Count {
    #[new]
    fn __new__(
        obj: &PyRawObject,
        root_path: &str,
        skip_hidden: Option<bool>,
        metadata: Option<bool>,
        metadata_ext: Option<bool>,
        max_depth: Option<usize>,
    ) {
        obj.init(Count {
            root_path: String::from(root_path),
            skip_hidden: skip_hidden.unwrap_or(false),
            metadata: metadata.unwrap_or(false),
            metadata_ext: metadata_ext.unwrap_or(false),
            max_depth: max_depth.unwrap_or(::std::usize::MAX),
            statistics: Arc::new(Mutex::new(Statistics { 
                dirs: 0,
                files: 0,
                slinks: 0,
                hlinks: 0,
                devices: 0,
                pipes: 0,
                size: 0,
                usage: 0,
                errors: Vec::new(),
            })),
            thr: None,
            alive: None,
            has_results: false,
            start_time: Instant::now(),
            duration: Duration::new(0, 0),
        });
    }

    #[getter]
    fn statistics(&self) -> PyResult<Statistics> {
       Ok(Arc::clone(&self.statistics).lock().unwrap().clone())
    }

    #[getter]
    fn duration(&self) -> PyResult<f64> {
       Ok(self.duration.as_secs() as f64 + self.duration.subsec_nanos() as f64 * 1e-9)
    }

    fn has_results(&self) -> PyResult<bool> {
        Ok(self.has_results)
     }
 
     fn as_dict(&self) -> PyResult<PyObject> {
        let gil = GILGuard::acquire();
        let stats_locked = self.statistics.lock().unwrap();
        let pyresult = PyDict::new(gil.python());
        if stats_locked.dirs > 0 {
            pyresult.set_item("dirs", stats_locked.dirs).unwrap();
        }
        if stats_locked.files > 0 {
            pyresult.set_item("files", stats_locked.files).unwrap();
        }
        if stats_locked.slinks > 0 {
            pyresult.set_item("slinks", stats_locked.slinks).unwrap();
        }
        if stats_locked.hlinks > 0 {
            pyresult.set_item("hlinks", stats_locked.hlinks).unwrap();
        }
        if stats_locked.devices > 0 {
            pyresult.set_item("devices", stats_locked.devices).unwrap();
        }
        if stats_locked.pipes > 0 {
            pyresult.set_item("pipes", stats_locked.pipes).unwrap();
        }
        if stats_locked.size > 0 {
            pyresult.set_item("size", stats_locked.size).unwrap();
        }
        if stats_locked.usage > 0 {
            pyresult.set_item("usage", stats_locked.usage).unwrap();
        }
        if !stats_locked.errors.is_empty() {
            pyresult.set_item("errors", stats_locked.errors.to_vec()).unwrap();
        }
        Ok(pyresult.to_object(gil.python()))
    }

    fn collect(&mut self) -> PyResult<Statistics> {
        self.start_time = Instant::now();
        rs_count(self.root_path.clone(),
                 self.skip_hidden, self.metadata, self.metadata_ext, self.max_depth,
                 self.statistics.clone(), None);
        self.duration = self.start_time.elapsed();
        self.has_results = true;
        Ok(Arc::clone(&self.statistics).lock().unwrap().clone())
    }

    fn start(&mut self) -> PyResult<bool> {
        if !self.rs_start() {
            return Err(exceptions::RuntimeError::py_err("Thread already running"))
        }
        Ok(true)
    }

    fn stop(&mut self) -> PyResult<bool> {
        if !self.rs_stop() {
            return Err(exceptions::RuntimeError::py_err("Thread not running"))
        }
        Ok(true)
    }

    fn busy(&self) -> PyResult<bool> {
        match &self.alive {
            Some(alive) => match alive.upgrade() {
                Some(_) => Ok(true),
                None => return Ok(false),
            },
            None => Ok(false),
        }
    }
}

#[pyproto]
impl pyo3::class::PyObjectProtocol for Count {
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}

#[pyproto]
impl<'p> PyContextProtocol<'p> for Count {
    fn __enter__(&'p mut self) -> PyResult<()> {
        self.rs_start();
        Ok(())
    }

    fn __exit__(
        &mut self,
        ty: Option<&'p PyType>,
        _value: Option<&'p PyAny>,
        _traceback: Option<&'p PyAny>,
    ) -> PyResult<bool> {
        if !self.rs_stop() {
            return Ok(false)
        }
        if ty == Some(GILGuard::acquire().python().get_type::<ValueError>()) {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[pymodule(count)]
fn init(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Count>()?;
    m.add_wrapped(wrap_pyfunction!(count))?;
    Ok(())
}

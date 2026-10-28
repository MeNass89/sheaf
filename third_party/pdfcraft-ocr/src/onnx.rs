//! Sheaf: the Latin recognition model run by onnxruntime, about twice rten's speed per core on
//! this model's convolutions. onnxruntime's prebuilt CPU library is linked statically where ort
//! publishes one (Linux x64 and arm64, macOS arm64, Windows x64: `build.rs`); elsewhere
//! [`Lines::load`] fails and rten reads the lines.

#[cfg(onnxruntime)]
pub(crate) use imp::Lines;

#[cfg(not(onnxruntime))]
pub(crate) struct Lines;

#[cfg(not(onnxruntime))]
impl Lines {
    pub(crate) fn load(_model: &[u8]) -> Result<Lines, String> {
        Err("onnxruntime is not built for this platform".into())
    }

    pub(crate) fn run(&self, _width: usize, _pixels: &[f32]) -> Result<(Vec<usize>, Vec<f32>), String> {
        Err("onnxruntime is not built for this platform".into())
    }
}

#[cfg(onnxruntime)]
mod imp {
    use crate::latin::HEIGHT;
    use ort::session::{Session, builder::{GraphOptimizationLevel, PrepackedWeights}};
    use std::sync::Mutex;

    /// Sessions of one model, one per line being read. Each runs on its calling thread only: the
    /// lines are already spread over every core (`read_latin`), so a pool per session would only
    /// make threads fight.
    pub(crate) struct Lines {
        env: ort::environment::Environment,
        model: Vec<u8>,
        /// The weights laid out for the convolutions once, shared by every session.
        packed: PrepackedWeights,
        /// Sessions not running now. A thread takes one, or makes one when none is free, and puts
        /// it back after: there are as many as lines were ever read at once.
        free: Mutex<Vec<Session>>,
    }

    fn err(e: impl std::fmt::Display) -> String {
        e.to_string()
    }

    impl Lines {
        pub(crate) fn load(model: &[u8]) -> Result<Lines, String> {
            // onnxruntime allows one environment a process, ever: made once, kept for every load.
            // No telemetry (Windows builds of onnxruntime send some by default).
            static ENV: std::sync::OnceLock<Result<ort::environment::Environment, String>> = std::sync::OnceLock::new();
            let env = ENV.get_or_init(|| ort::init().with_name("sheaf-ocr").with_telemetry(false).build().map_err(err)).clone()?;
            let lines = Lines { env, model: model.to_vec(), packed: PrepackedWeights::new(), free: Mutex::new(Vec::new()) };
            // One session made now, so a model onnxruntime cannot run falls back to rten at load.
            let first = lines.session()?;
            lines.free.lock().map_err(err)?.push(first);
            Ok(lines)
        }

        fn session(&self) -> Result<Session, String> {
            Session::builder(&self.env)
                .map_err(err)?
                .with_optimization_level(GraphOptimizationLevel::All)
                .map_err(err)?
                .with_intra_threads(1)
                .map_err(err)?
                .with_inter_threads(1)
                .map_err(err)?
                .with_prepacked_weights(&self.packed)
                .map_err(err)?
                // Every line has its own width: a memory plan made for one shape fits no other.
                .with_memory_pattern(false)
                .map_err(err)?
                .commit_from_memory(&self.model)
                .map_err(err)
        }

        /// Runs the model on one line, `[1, 3, 48, width]`: the output's shape and its values.
        pub(crate) fn run(&self, width: usize, pixels: &[f32]) -> Result<(Vec<usize>, Vec<f32>), String> {
            let taken = self.free.lock().map_err(err)?.pop();
            let mut session = match taken {
                Some(s) => s,
                None => self.session()?,
            };
            let out = (|| {
                let input = ort::value::TensorRef::from_array_view(([1usize, 3, HEIGHT, width], pixels)).map_err(err)?;
                let outputs = session.run(ort::inputs![input]).map_err(err)?;
                let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(err)?;
                Ok((shape.iter().map(|d| *d as usize).collect(), values.to_vec()))
            })();
            self.free.lock().map_err(err)?.push(session);
            out
        }
    }
}

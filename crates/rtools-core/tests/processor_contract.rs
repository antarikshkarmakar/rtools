use rtools_core::{Processor, RToolsError, RToolsResult};
use std::sync::atomic::{AtomicBool, Ordering};
use {derive_more as _, dirs as _, figment as _, serde as _, serde_json as _, thiserror as _};
use {toml as _, tracing as _};

struct RejectingProcessor {
    ran: AtomicBool,
}

impl Processor for RejectingProcessor {
    type Input = ();
    type Output = ();
    type Config = ();
    type Error = RToolsError;

    fn process_validated(&self, _input: (), _config: ()) -> RToolsResult<()> {
        self.ran.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn validate_config(&self, _config: &()) -> RToolsResult<()> {
        Err(RToolsError::configuration_invalid("rejected by test"))
    }

    fn name(&self) -> &'static str {
        "RejectingProcessor"
    }
}

#[test]
fn process_never_runs_when_validation_fails() {
    let processor = RejectingProcessor {
        ran: AtomicBool::new(false),
    };

    let error = processor.process((), ()).unwrap_err();

    assert_eq!(error.code().as_str(), "CONFIGURATION_INVALID");
    assert!(!processor.ran.load(Ordering::SeqCst));
}

//! Progress reporting for segmented downloads.
//!
//! [`ProgressSink`] is a cloneable callback handle rather than a trait:
//! spawned segment tasks need an owned, `'static` reporter, and a
//! callback handle can be cloned cheaply from a plain `&ProgressSink`
//! parameter without extra plumbing. [`ProgressEvent`] carries one
//! event per segment attempt; [`BytesTracker`] aggregates the bytes
//! finished across all segments.

#![allow(clippy::module_name_repetitions)] // `Progress*` types read naturally inside `mod progress`

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::types::{ExitId, SegmentIndex};

/// One event describing the state of a segment download attempt.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// A segment attempt started (attempt 1 is the initial try).
    SegmentStarted {
        index: SegmentIndex,
        exit: ExitId,
        attempt: u32,
    },
    /// A segment finished successfully. `duration` spans the whole
    /// segment, all attempts (retries included).
    SegmentFinished {
        index: SegmentIndex,
        exit: ExitId,
        bytes: u64,
        duration: Duration,
    },
    /// A segment attempt failed. `error` is the formatted failure
    /// reason; the typed error surfaces through the fetch's result.
    SegmentFailed {
        index: SegmentIndex,
        exit: ExitId,
        attempt: u32,
        error: String,
    },
}

impl fmt::Display for ProgressEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SegmentStarted {
                index,
                exit,
                attempt,
            } => {
                write!(f, "segment {index} on {exit}: attempt {attempt} started")
            }
            Self::SegmentFinished {
                index,
                exit,
                bytes,
                duration,
            } => {
                write!(
                    f,
                    "segment {index} on {exit}: {bytes} bytes in {duration:.2?}"
                )
            }
            Self::SegmentFailed {
                index,
                exit,
                attempt,
                error,
            } => {
                write!(
                    f,
                    "segment {index} on {exit}: attempt {attempt} failed: {error}"
                )
            }
        }
    }
}

/// Cloneable handle to a progress callback; `Send + Sync`, so it can be
/// shared across tokio tasks. All clones call the same callback.
#[derive(Clone)]
pub struct ProgressSink {
    on_event: Arc<dyn Fn(&ProgressEvent) + Send + Sync>,
}

impl ProgressSink {
    /// Wrap any event callback into a sink.
    pub fn new<F>(on_event: F) -> Self
    where
        F: Fn(&ProgressEvent) + Send + Sync + 'static,
    {
        Self {
            on_event: Arc::new(on_event),
        }
    }

    /// A sink that discards every event.
    #[must_use]
    pub fn noop() -> Self {
        Self::new(|_| ())
    }

    /// A sink that logs every event through `tracing`: started attempts
    /// at debug, finished segments at info, failures at warn.
    #[must_use]
    pub fn tracing_log() -> Self {
        Self::new(|event| match event {
            ProgressEvent::SegmentStarted { .. } => tracing::debug!("{event}"),
            ProgressEvent::SegmentFinished { .. } => tracing::info!("{event}"),
            ProgressEvent::SegmentFailed { .. } => tracing::warn!("{event}"),
        })
    }

    /// Combine two sinks: both receive every event.
    #[must_use]
    pub fn fanout(first: &Self, second: &Self) -> Self {
        let first = first.clone();
        let second = second.clone();
        Self::new(move |event| {
            first.on_event(event);
            second.on_event(event);
        })
    }

    /// Deliver one event to the underlying callback.
    pub fn on_event(&self, event: &ProgressEvent) {
        (self.on_event)(event);
    }
}

impl fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgressSink").finish_non_exhaustive()
    }
}

/// Counts the bytes of successfully finished segments across the whole
/// fetch. Cheap to clone: every clone observes the same total.
#[derive(Debug, Default, Clone)]
pub struct BytesTracker {
    bytes: Arc<AtomicU64>,
}

impl BytesTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Total bytes reported as finished so far.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// A sink that feeds this tracker; the tracker stays queryable.
    #[must_use]
    pub fn sink(&self) -> ProgressSink {
        let tracker = self.clone();
        ProgressSink::new(move |event| tracker.record(event))
    }

    fn record(&self, event: &ProgressEvent) {
        if let ProgressEvent::SegmentFinished { bytes, .. } = event {
            self.bytes.fetch_add(*bytes, Ordering::Relaxed);
        }
    }
}

//! B-Thread coordination layer for SCUD (scud-weave).
//!
//! Implements David Harel's behavioral programming model:
//! request/wait/block at synchronization points.

pub mod bthread;
pub mod coordinator;
pub mod event;
pub mod log;
pub mod matcher;

// Re-export primary types
pub use bthread::{
    BThread, BThreadRule, NodeAnnotation, PartitionDef, PartitionStrategy, Role, TimeoutAction,
    WeaveEdge, WeaveEdgeType,
};
pub use coordinator::{ActiveLock, Coordinator, Decision};
pub use event::{Event, EventKind, EventPattern};
pub use log::{EventLog, TimestampedEvent};
pub use matcher::GlobPattern;

//! PhotoSee core: everything testable without a UI or a network stack.
//! See docs/api.md for the module ownership map and contracts.

pub mod archive;
pub mod color;
pub mod config;
pub mod decode;
pub mod delivery;
pub mod encode;
#[cfg(windows)]
pub mod encode_wic;
pub mod error;
pub mod format;
pub mod io;
pub mod meta;
pub mod nav;
pub mod orient;
pub mod pipeline;
pub mod resize;
pub mod scan;
pub mod sort;
pub mod thumb;
pub mod thumbcache;
pub mod types;

#[cfg(any(test, feature = "testing"))]
pub mod testfs;

pub use error::{Error, ErrorCode, Result};
pub use types::*;

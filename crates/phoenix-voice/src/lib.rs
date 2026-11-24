pub mod audio;
pub mod codec;
pub mod connection;
pub mod crypto;
pub mod dave;
pub mod error;
pub mod gateway;
#[cfg(target_os = "linux")]
pub mod hotkey;
pub mod leb128;
pub mod media;
pub mod rtp;
pub mod session;
pub mod udp;

pub use error::{Error, Result};

pub const FRAME_SAMPLES: usize = 960;

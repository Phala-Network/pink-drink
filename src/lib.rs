pub use drink;

pub use error::{Error, Result};
pub use ink_helper::{code_hash, Callable, DeployBundle, Deployable, SessionExt};
pub use runtime::PinkRuntime;

mod error;
mod runtime;
mod types;

mod blocking;
mod ink_helper;
mod sidevm_runner;

pub fn version() -> (u32, u32) {
    let major = env!("CARGO_PKG_VERSION_MAJOR")
        .parse()
        .expect("Invalid major version");

    let minor = env!("CARGO_PKG_VERSION_MINOR")
        .parse()
        .expect("Invalid minor version");

    (major, minor)
}

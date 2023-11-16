pub use drink;

pub use ink_helper::{Callable, DeployBundle, Deployable, SessionExt};
pub use runtime::PinkRuntime;

mod runtime;
mod types;

mod ink_helper;

pub fn version() -> (u32, u32) {
    let major = env!("CARGO_PKG_VERSION_MAJOR")
        .parse()
        .expect("Invalid major version");

    let minor = env!("CARGO_PKG_VERSION_MINOR")
        .parse()
        .expect("Invalid minor version");

    (major, minor)
}

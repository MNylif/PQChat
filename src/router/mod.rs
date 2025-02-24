mod layers;
mod request;
mod router;
mod run;
mod serve;

extern crate pqchat_core as pqchat;

use std::{panic::AssertUnwindSafe, pin::Pin, sync::Arc};

use pqchat::{Error, Result, Server};
use pqchat_service::Services;
use futures::{Future, FutureExt, TryFutureExt};

pqchat::mod_ctor! {}
pqchat::mod_dtor! {}
pqchat::rustc_flags_capture! {}

#[unsafe(no_mangle)]
pub extern "Rust" fn start(
	server: &Arc<Server>,
) -> Pin<Box<dyn Future<Output = Result<Arc<Services>>> + Send>> {
	AssertUnwindSafe(run::start(server.clone()))
		.catch_unwind()
		.map_err(Error::from_panic)
		.unwrap_or_else(Err)
		.boxed()
}

#[unsafe(no_mangle)]
pub extern "Rust" fn stop(
	services: Arc<Services>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
	AssertUnwindSafe(run::stop(services))
		.catch_unwind()
		.map_err(Error::from_panic)
		.unwrap_or_else(Err)
		.boxed()
}

#[unsafe(no_mangle)]
pub extern "Rust" fn run(
	services: &Arc<Services>,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
	AssertUnwindSafe(run::run(services.clone()))
		.catch_unwind()
		.map_err(Error::from_panic)
		.unwrap_or_else(Err)
		.boxed()
}

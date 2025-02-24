use std::{net::SocketAddr, sync::Arc};

use axum::Router;
use axum_server::Handle as ServerHandle;
use axum_server_dual_protocol::{
	axum_server::{bind_rustls, tls_rustls::RustlsConfig},
	ServerExt,
};
use conduwuit::{err, Result, Server};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use crate::crypto::CryptoState;

pub(super) async fn serve(
	server: &Arc<Server>,
	app: Router,
	handle: ServerHandle,
	addrs: Vec<SocketAddr>,
) -> Result {
	let tls = &server.config.tls;
	let conf = load_tls_config();

	let mut join_set = JoinSet::new();
	let app = app.into_make_service_with_connect_info::<SocketAddr>();
	if tls.dual_protocol {
		for addr in &addrs {
			join_set.spawn_on(
				axum_server_dual_protocol::bind_dual_protocol(*addr, conf.clone())
					.set_upgrade(false)
					.handle(handle.clone())
					.serve(app.clone()),
				server.runtime(),
			);
		}
	} else {
		for addr in &addrs {
			join_set.spawn_on(
				bind_rustls(*addr, conf.clone())
					.handle(handle.clone())
					.serve(app.clone()),
				server.runtime(),
			);
		}
	}

	if tls.dual_protocol {
		warn!(
			"Listening on {addrs:?} with TLS certificate and supporting plain text \
			 (HTTP) connections too (insecure!)",
		);
	} else {
		info!("Listening on {addrs:?} with TLS certificate");
	}

	while join_set.join_next().await.is_some() {}

	Ok(())
}

pub fn load_tls_config() -> Arc<rustls::ServerConfig> {
    // Initialize the crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to initialise aws-lc-rs rustls crypto provider");

    let crypto_state = CryptoState::new();

    let cert = tls.certs.as_ref().ok_or_else(|| {
		err!(Config("tls.certs", "Missing required value in tls config section"))
	})?;
	let key = tls
		.key
		.as_ref()
		.ok_or_else(|| err!(Config("tls.key", "Missing required value in tls config section")))?;

    let mut config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth();

    // Enable post-quantum KEM groups
    config.key_share_groups = vec![
        rustls::NamedGroup::Kyber768,  // Post-quantum
        rustls::NamedGroup::X25519,    // Classical fallback
    ];

    // Set supported protocol versions
    config.versions = vec![rustls::ProtocolVersion::TLSv1_3];

    config.cert_resolver = Arc::new(SingleCertResolver::new(cert, key));

    Arc::new(config)
}

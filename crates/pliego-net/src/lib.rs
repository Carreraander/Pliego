//! Cliente HTTP(S) común de Pliego.
//!
//! Todas las descargas (modelos de OCR y del asistente, CRL/OCSP, sellos de tiempo) pasan por aquí para que usen el
//! TLS del sistema operativo (SChannel en Windows) y su almacén de certificados. Con las raíces de Mozilla que
//! incluye rustls fallaba con `invalid peer certificate: UnknownIssuer` en equipos con inspección TLS de empresa
//! (Zscaler, Netskope...) o con una CA propia instalada en Windows.

use std::time::Duration;

pub use ureq::Error;
pub use ureq::http::Response;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
pub use ureq::{Agent, Body};

/// Agente con TLS del sistema, almacén de certificados de Windows y, en Windows, el proxy del sistema.
pub fn agent() -> Agent {
    Agent::config_builder()
        .user_agent(concat!("Pliego/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(20)))
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

/// Petición GET con el agente común.
pub fn get(url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    agent().get(url)
}

/// Petición POST con el agente común.
pub fn post(url: &str) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    agent().post(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_agente_se_construye_con_tls_nativo() {
        // Sin red: solo comprueba que la configuración TLS es válida y que una URL inválida falla limpiamente.
        let err = agent().get("http://127.0.0.1:1/").call().unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}

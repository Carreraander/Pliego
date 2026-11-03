//! Despliegue gradual decidido en el propio equipo.

use sha2::{Digest, Sha256};

/// Cubo 0–99 estable para una instalación y una versión. Cambia con cada versión para que no sean siempre
/// los mismos equipos los primeros en recibirlas.
pub fn bucket(install_id: &str, version: &semver::Version) -> u8 {
    let mut h = Sha256::new();
    h.update(install_id.as_bytes());
    h.update(b"/");
    h.update(version.to_string().as_bytes());
    let d = h.finalize();
    (u16::from_be_bytes([d[0], d[1]]) % 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_is_stable_and_spread() {
        let v = semver::Version::new(1, 2, 3);
        assert_eq!(bucket("a", &v), bucket("a", &v));
        let mut seen = [0u32; 100];
        for i in 0..10_000 {
            seen[bucket(&format!("install-{i}"), &v) as usize] += 1;
        }
        // Reparto razonable: ningún cubo vacío ni con más del triple de la media.
        assert!(seen.iter().all(|&n| n > 0 && n < 300), "{seen:?}");
    }
}

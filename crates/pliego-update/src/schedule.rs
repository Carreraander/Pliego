//! Cuándo comprobar: 30 s tras arrancar, luego cada 6 h, como mucho una vez por hora.

use time::{Duration, OffsetDateTime};

pub const STARTUP_DELAY: Duration = Duration::seconds(30);
pub const INTERVAL: Duration = Duration::hours(6);
/// Separación mínima entre dos intentos, también tras un fallo ("Se reintentará en 1 hora").
pub const MIN_GAP: Duration = Duration::hours(1);

/// Resultado del último intento de esta sesión.
#[derive(Debug, Clone, Copy)]
pub struct Attempt {
    pub at: OffsetDateTime,
    pub ok: bool,
}

/// Instante de la próxima comprobación. Función pura: el llamador duerme hasta ese momento.
///
/// - `started`: arranque de la app. - `last_ok`: última comprobación correcta guardada en disco (de sesiones previas).
/// - `last`: último intento de esta sesión, si lo hubo.
pub fn next_check(started: OffsetDateTime, last_ok: Option<OffsetDateTime>, last: Option<Attempt>) -> OffsetDateTime {
    match last {
        Some(a) if a.ok => a.at + INTERVAL,
        Some(a) => a.at + MIN_GAP,
        None => {
            let first = started + STARTUP_DELAY;
            match last_ok {
                Some(t) => first.max(t + MIN_GAP),
                None => first,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn schedule_follows_the_design() {
        let t0 = datetime!(2026-11-03 10:00 UTC);
        assert_eq!(next_check(t0, None, None), t0 + Duration::seconds(30));
        // Comprobó hace 10 min en la sesión anterior: espera a que pase la hora.
        let recent = Some(t0 - Duration::minutes(10));
        assert_eq!(next_check(t0, recent, None), t0 + Duration::minutes(50));
        // Comprobó hace 3 h: basta con los 30 s.
        let old = Some(t0 - Duration::hours(3));
        assert_eq!(next_check(t0, old, None), t0 + Duration::seconds(30));
        let ok = Attempt { at: t0, ok: true };
        assert_eq!(next_check(t0, None, Some(ok)), t0 + Duration::hours(6));
        let bad = Attempt { at: t0, ok: false };
        assert_eq!(next_check(t0, None, Some(bad)), t0 + Duration::hours(1));
    }
}

// Test-only Rust executable: deterministic mathematical positions, NOT ephemerides.
use std::io::{self, BufRead, Write};

fn main() {
    let name = std::env::var("MOCK_PROVIDER_NAME").unwrap_or_else(|_| "synthetic-test".into());
    let offset = std::env::var("MOCK_LONGITUDE_OFFSET")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    println!("{{\"protocol\":\"virgostars.longitude.v2\",\"provider\":\"{name}\",\"version\":\"1\",\"frame\":\"geocentric-tropical-ecliptic-degrees\",\"input_time_scale\":\"utc-posix-microseconds\",\"flags\":\"fake-linear\",\"kernel_digest\":null}}");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let request = line.unwrap();
        let requested_body = request
            .split("\"body\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let at: i64 = request
            .split("\"utc_microseconds\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let days = at as f64 / 86_400_000_000_f64;
        let (longitude, speed) = match requested_body {
            "Sun" => (10.0 + days, 1.0),
            "Moon" if std::env::var_os("MOCK_LUNAR_ALIAS").is_some() => {
                (20.0 + 731.0 * days, 731.0)
            }
            "Moon" => (10.0 + 13.0 * days, 13.0),
            _ => (29.0 + days * 2.0, 2.0),
        };
        // Synthetic forward Taurus ingress followed by retrograde Aries
        // re-entry, both concealed by the same Aries longitude at day edges.
        let (longitude, speed) = if requested_body == "Mars"
            && std::env::var_os("MOCK_DOUBLE_INGRESS").is_some()
        {
            (
                29.0 + 8.0 * (0.25 - (days - 0.5).powi(2)),
                -16.0 * (days - 0.5),
            )
        } else {
            (longitude, speed)
        };
        // Two stations strictly inside a daily scan step: the daily endpoints
        // have equal speed sign, but a half-day sample exposes both crossings.
        let speed = if requested_body == "Mars"
            && std::env::var_os("MOCK_DOUBLE_STATION").is_some()
        {
            (days - 0.25) * (days - 0.75)
        } else {
            speed
        };
        let body = if std::env::var_os("MOCK_BAD_ECHO").is_some() {
            "Venus"
        } else {
            requested_body
        };
        println!(
            "{{\"body\":\"{body}\",\"utc_microseconds\":\"{at}\",\"longitude_degrees\":{},\"longitude_speed_degrees_per_day\":{speed}}}",
            longitude + if body == "Mars" { offset } else { 0.0 }
        );
        io::stdout().flush().unwrap();
    }
}

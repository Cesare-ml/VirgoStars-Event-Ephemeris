// Test-only Rust executable: deterministic mathematical positions, NOT ephemerides.
use std::io::{self, BufRead, Write};

fn main() {
    println!("{{\"protocol\":\"virgostars.longitude.v2\",\"provider\":\"synthetic-test\",\"version\":\"1\",\"frame\":\"geocentric-tropical-ecliptic-degrees\",\"input_time_scale\":\"utc-posix-microseconds\",\"flags\":\"fake-linear\",\"kernel_digest\":null}}");
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
            "Moon" => (10.0 + 13.0 * days, 13.0),
            _ => (29.0 + days * 2.0, 2.0),
        };
        let body = if std::env::var_os("MOCK_BAD_ECHO").is_some() {
            "Venus"
        } else {
            requested_body
        };
        println!(
            "{{\"body\":\"{body}\",\"utc_microseconds\":\"{at}\",\"longitude_degrees\":{longitude},\"longitude_speed_degrees_per_day\":{speed}}}"
        );
        io::stdout().flush().unwrap();
    }
}

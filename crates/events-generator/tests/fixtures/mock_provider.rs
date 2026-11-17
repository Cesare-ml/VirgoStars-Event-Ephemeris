// Test-only Rust executable: deterministic mathematical positions, NOT ephemerides.
use std::io::{self, BufRead, Write};

fn main() {
    println!("{{\"protocol\":\"virgostars.longitude.v1\",\"provider\":\"synthetic-test\",\"version\":\"1\",\"frame\":\"geocentric-tropical-ecliptic-degrees\",\"input_time_scale\":\"utc-posix-microseconds\",\"flags\":\"fake-linear\",\"kernel_digest\":null}}");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let request = line.unwrap();
        let at: i64 = request
            .split("\"utc_microseconds\":")
            .nth(1)
            .unwrap()
            .trim_end_matches('}')
            .parse()
            .unwrap();
        let longitude = 29.0 + (at as f64 / 86_400_000_000_f64) * 2.0;
        let body = if std::env::var_os("MOCK_BAD_ECHO").is_some() {
            "Venus"
        } else {
            "Mars"
        };
        println!(
            "{{\"body\":\"{body}\",\"utc_microseconds\":{at},\"longitude_degrees\":{longitude},\"longitude_speed_degrees_per_day\":2.0}}"
        );
        io::stdout().flush().unwrap();
    }
}

/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
// Test-only executable; analytical constants are NOT planetary ephemerides.
use std::io::{self, BufRead, Write};

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split(&format!("\"{key}\":\"")).nth(1)?.split('"').next()
}

fn main() {
    let mode = std::env::current_exe()
        .unwrap()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    if mode.contains("args_required")
        && std::env::args().skip(1).collect::<Vec<_>>() != ["--source", "path with spaces"]
    {
        eprintln!("literal source arguments were not forwarded as separate argv entries");
        std::process::exit(77);
    }
    let mut identity = String::from(
        r#"{"protocol":"virgostars.latitude.v1","provider":"synthetic-latitude-fixture","version":"1","frame":"geocentric-ecliptic-latitude-degrees","ecliptic_plane":"mean-of-date","position_corrections":"geometric","dynamical_time_scale":"TT","input_time_scale":"utc-posix-microseconds","speed_definition":"signed-d-beta-dt-degrees-per-day","flags":"synthetic-only","source_license":"test-fixture","kernel_digest":null,"source_digest":null}"#,
    );
    if mode.contains("bad_protocol") {
        identity = identity.replace("virgostars.latitude.v1", "virgostars.longitude.v2");
    } else if mode.contains("missing_plane") {
        identity = identity.replace("\"mean-of-date\"", "\"\"");
    } else if mode.contains("different_plane") {
        identity = identity.replace("\"mean-of-date\"", "\"true-of-date\"");
    }
    println!("{identity}");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let request = line.unwrap();
        let body = field(&request, "body").unwrap_or("?");
        let at = field(&request, "utc_microseconds").unwrap_or("?");
        if mode.contains("disconnect") {
            return;
        }
        if mode.contains("oversize") {
            println!("{}", "x".repeat(9_000));
        } else {
            let response_body = if mode.contains("wrong_body") { "Moon" } else { body };
            let response_time = if mode.contains("wrong_time") { "0" } else { at };
            let (latitude, speed) = if mode.contains("analytic") {
                let days = at.parse::<i64>().unwrap() as f64 / 86_400_000_000.0;
                let phase = 2.0 * std::f64::consts::PI * (days + 0.1);
                let beta = 0.3 * phase.cos();
                let speed = -0.3 * 2.0 * std::f64::consts::PI * phase.sin();
                (beta.to_string(), speed.to_string())
            } else {
                let beta = if mode.contains("invalid_latitude") {
                    "91.0"
                } else if mode.contains("shifted") {
                    "1.75"
                } else {
                    "1.25"
                };
                let velocity = if mode.contains("invalid_speed") {
                    "1e999"
                } else {
                    "-0.125"
                };
                (beta.to_string(), velocity.to_string())
            };
            println!(
                "{{\"body\":\"{response_body}\",\"utc_microseconds\":\"{response_time}\",\"latitude_degrees\":{latitude},\"latitude_speed_degrees_per_day\":{speed}}}"
            );
        }
        io::stdout().flush().unwrap();
    }
}

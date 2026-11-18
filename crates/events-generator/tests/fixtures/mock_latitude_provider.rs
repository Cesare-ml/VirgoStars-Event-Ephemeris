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
    let mut identity = String::from(
        r#"{"protocol":"virgostars.latitude.v1","provider":"synthetic-latitude-fixture","version":"1","frame":"geocentric-ecliptic-latitude-degrees","ecliptic_plane":"mean-of-date","position_corrections":"geometric","dynamical_time_scale":"TT","input_time_scale":"utc-posix-microseconds","speed_definition":"signed-d-beta-dt-degrees-per-day","flags":"synthetic-only","source_license":"test-fixture","kernel_digest":null,"source_digest":null}"#,
    );
    if mode.contains("bad_protocol") {
        identity = identity.replace("virgostars.latitude.v1", "virgostars.longitude.v2");
    } else if mode.contains("missing_plane") {
        identity = identity.replace("\"mean-of-date\"", "\"\"");
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
            let latitude = if mode.contains("invalid_latitude") {
                "91.0"
            } else {
                "1.25"
            };
            let speed = if mode.contains("invalid_speed") {
                "1e999"
            } else {
                "-0.125"
            };
            println!(
                "{{\"body\":\"{response_body}\",\"utc_microseconds\":\"{response_time}\",\"latitude_degrees\":{latitude},\"latitude_speed_degrees_per_day\":{speed}}}"
            );
        }
        io::stdout().flush().unwrap();
    }
}

/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (the "License"); you may not use this file except in
 * compliance with the License. You may obtain a copy at
 * https://opensource.org/license/CPAL-1.0.
 *
 * The Original Code is VirgoStars-Event-Ephemeris source code.
 * The Original Developer and Initial Developer is Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY KIND.
 * See repository LICENSE (Exhibits A and B) and NOTICE.
 *
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Typed, independent foundations for astronomical event ephemerides.
//!
//! This crate models precomputed synods, motion stations and sign ingresses.
//! It does not calculate astronomical events, decode a dataset or open PostgreSQL.
//! Search indexes, file-backed readers and cross-language bindings are future work.

use std::error::Error;
use std::fmt;

pub mod coverage;
pub mod indexed;
pub mod synod_stage;
pub use coverage::{CoverageAssessment, CoverageError, CoveragePolicy, ReviewedWindow};
pub use indexed::{
    ChronologicalCursor, ChronologicalPage, ChronologicalQueryError, EventFamily, EventIndex,
    IndexError, QueryError, SynodHandle, MAX_CHRONOLOGICAL_PAGE_SIZE,
};

pub use synod_stage::{
    assess_synod_stage, SynodStage, SynodStageAssessment, SynodStageStatus,
    MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_MICROS,
};

pub use virgostars_events_format::{probe_magic, FormatProbeError, VSEPH_MAGIC};

/// A signed microsecond count from the Unix/POSIX UTC epoch.
///
/// This represents an absolute timestamp, **not** TT or TDB, and deliberately
/// avoids floating-point conversions or a system time-zone dependency.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimestampMicros(i64);

impl TimestampMicros {
    pub const fn new(microseconds: i64) -> Self {
        Self(microseconds)
    }

    pub const fn as_i64(self) -> i64 {
        self.0
    }
}

/// An exact nonempty body identifier. Original spelling and case are preserved.
///
/// Body IDs are deliberately not limited to conventional planet names: source
/// tables also include values such as "true Node" and "osc. Apogee".
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BodyId(String);

impl BodyId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for BodyId {
    type Error = ModelError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        if input.is_empty() || input.trim() != input || input.chars().any(char::is_control) {
            return Err(ModelError::InvalidBody);
        }
        Ok(Self(input.to_owned()))
    }
}

impl fmt::Display for BodyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Zero-based zodiac sign: Aries=0 through Pisces=11.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ZodiacSign(u8);

impl ZodiacSign {
    pub const fn index(self) -> u8 {
        self.0
    }
}

impl TryFrom<i32> for ZodiacSign {
    type Error = ModelError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if !(0..=11).contains(&value) {
            return Err(ModelError::InvalidSign(value));
        }
        Ok(Self(value as u8))
    }
}

impl TryFrom<u8> for ZodiacSign {
    type Error = ModelError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::try_from(i32::from(value))
    }
}

/// Direction *after* a motion-station event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MotionDirection {
    Direct,
    Retrograde,
}

impl MotionDirection {
    /// Matches the source table's one-character `moto` field.
    pub fn from_legacy_code(value: &str) -> Result<Self, ModelError> {
        match value {
            "d" => Ok(Self::Direct),
            "r" => Ok(Self::Retrograde),
            _ => Err(ModelError::InvalidMotionCode),
        }
    }

    pub const fn legacy_code(self) -> &'static str {
        match self {
            Self::Direct => "d",
            Self::Retrograde => "r",
        }
    }
}

/// Invalid source-model values. No I/O or dataset coverage errors are implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    InvalidBody,
    InvalidSign(i32),
    InvalidMotionCode,
    InvalidPhaseName,
    InvalidPhaseOrder,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBody => f.write_str("invalid celestial body identifier"),
            Self::InvalidSign(sign) => write!(f, "invalid zodiac sign index: {sign}"),
            Self::InvalidMotionCode => f.write_str("invalid motion code; expected d or r"),
            Self::InvalidPhaseName => f.write_str("invalid synodic phase name"),
            Self::InvalidPhaseOrder => {
                f.write_str("synodic phases must be strictly after the anchor and increasing")
            }
        }
    }
}

impl Error for ModelError {}

/// Named additional phase of a synod, after the cycle anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynodPhase {
    name: String,
    at: TimestampMicros,
}

impl SynodPhase {
    pub fn new(name: impl Into<String>, at: TimestampMicros) -> Result<Self, ModelError> {
        let name = name.into();
        if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
            return Err(ModelError::InvalidPhaseName);
        }
        Ok(Self { name, at })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn at(&self) -> TimestampMicros {
        self.at
    }
}

/// A precomputed synod anchor and its optional additional phases.
///
/// `None` means phases are absent/unknown; `Some(vec![])` means an explicitly
/// empty phase list. Neither implies a specific number of lunar/planetary phases.
/// The cycle's end is unknown until a later anchor is loaded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynodEvent {
    body: BodyId,
    start: TimestampMicros,
    phases: Option<Vec<SynodPhase>>,
}

impl SynodEvent {
    pub fn new(
        body: BodyId,
        start: TimestampMicros,
        phases: Option<Vec<SynodPhase>>,
    ) -> Result<Self, ModelError> {
        if let Some(ref list) = phases {
            let mut previous = start;
            for phase in list {
                if phase.at <= previous {
                    return Err(ModelError::InvalidPhaseOrder);
                }
                previous = phase.at;
            }
        }
        Ok(Self {
            body,
            start,
            phases,
        })
    }

    pub fn body(&self) -> &BodyId {
        &self.body
    }

    pub const fn start(&self) -> TimestampMicros {
        self.start
    }

    pub fn phases(&self) -> Option<&[SynodPhase]> {
        self.phases.as_deref()
    }
}

/// A change of motion direction at a known instant; includes the recorded sign.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MotionEvent {
    body: BodyId,
    at: TimestampMicros,
    direction: MotionDirection,
    sign: ZodiacSign,
}

impl MotionEvent {
    pub fn new(
        body: BodyId,
        at: TimestampMicros,
        direction: MotionDirection,
        sign: ZodiacSign,
    ) -> Self {
        Self {
            body,
            at,
            direction,
            sign,
        }
    }

    pub fn body(&self) -> &BodyId {
        &self.body
    }

    pub const fn at(&self) -> TimestampMicros {
        self.at
    }

    pub const fn direction(&self) -> MotionDirection {
        self.direction
    }

    pub const fn sign(&self) -> ZodiacSign {
        self.sign
    }
}

/// An ingress into the recorded sign at a known instant.
///
/// A body may enter the same sign again after a retrograde re-entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngressEvent {
    body: BodyId,
    at: TimestampMicros,
    sign: ZodiacSign,
}

impl IngressEvent {
    pub fn new(body: BodyId, at: TimestampMicros, sign: ZodiacSign) -> Self {
        Self { body, at, sign }
    }

    pub fn body(&self) -> &BodyId {
        &self.body
    }

    pub const fn at(&self) -> TimestampMicros {
        self.at
    }

    pub const fn sign(&self) -> ZodiacSign {
        self.sign
    }
}

/// One event in the future unified chronological query iterator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Synod(SynodEvent),
    Motion(MotionEvent),
    Ingress(IngressEvent),
}

impl Event {
    pub fn body(&self) -> &BodyId {
        match self {
            Self::Synod(event) => event.body(),
            Self::Motion(event) => event.body(),
            Self::Ingress(event) => event.body(),
        }
    }

    pub fn at(&self) -> TimestampMicros {
        match self {
            Self::Synod(event) => event.start(),
            Self::Motion(event) => event.at(),
            Self::Ingress(event) => event.at(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(name: &str) -> BodyId {
        BodyId::try_from(name).expect("valid body")
    }

    fn phase(name: &str, at: i64) -> SynodPhase {
        SynodPhase::new(name, TimestampMicros::new(at)).expect("valid phase")
    }

    #[test]
    fn body_ids_preserve_source_spelling_and_reject_invalid_input() {
        for name in [
            "Moon",
            "Mercury",
            "true Node",
            "true Node Sud",
            "osc. Apogee",
        ] {
            assert_eq!(body(name).as_str(), name);
        }
        for invalid in ["", " Moon", "Moon ", "Moon\n", "\t"] {
            assert_eq!(BodyId::try_from(invalid), Err(ModelError::InvalidBody));
        }
    }

    #[test]
    fn zodiac_signs_are_strictly_zero_to_eleven() {
        assert_eq!(ZodiacSign::try_from(0_i32).unwrap().index(), 0);
        assert_eq!(ZodiacSign::try_from(11_i32).unwrap().index(), 11);
        assert_eq!(
            ZodiacSign::try_from(-1_i32),
            Err(ModelError::InvalidSign(-1))
        );
        assert_eq!(
            ZodiacSign::try_from(12_i32),
            Err(ModelError::InvalidSign(12))
        );
        assert_eq!(
            ZodiacSign::try_from(255_u8),
            Err(ModelError::InvalidSign(255))
        );
    }

    #[test]
    fn direction_round_trips_legacy_codes() {
        assert_eq!(
            MotionDirection::from_legacy_code("d").unwrap(),
            MotionDirection::Direct
        );
        assert_eq!(
            MotionDirection::from_legacy_code("r").unwrap(),
            MotionDirection::Retrograde
        );
        assert_eq!(MotionDirection::Direct.legacy_code(), "d");
        assert_eq!(MotionDirection::Retrograde.legacy_code(), "r");
        assert_eq!(
            MotionDirection::from_legacy_code("R"),
            Err(ModelError::InvalidMotionCode)
        );
    }

    #[test]
    fn signed_microseconds_do_not_lose_pre_epoch_precision() {
        let instant = TimestampMicros::new(-876_544);
        assert_eq!(instant.as_i64(), -876_544);
        assert!(instant < TimestampMicros::new(0));
        assert_eq!(TimestampMicros::new(1).as_i64(), 1);
    }

    #[test]
    fn optional_phases_distinguish_unknown_from_explicitly_empty() {
        let missing = SynodEvent::new(body("true Node"), TimestampMicros::new(0), None).unwrap();
        let empty = SynodEvent::new(body("Moon"), TimestampMicros::new(0), Some(vec![])).unwrap();
        assert_eq!(missing.phases(), None);
        assert!(empty.phases().expect("explicit empty phases").is_empty());
    }

    #[test]
    fn synod_accepts_variable_phases_but_requires_strict_order() {
        let two = SynodEvent::new(
            body("osc. Apogee"),
            TimestampMicros::new(-10),
            Some(vec![phase("conjunction2", -5), phase("conjunction3", -1)]),
        )
        .unwrap();
        assert_eq!(two.phases().unwrap().len(), 2);
        let three = SynodEvent::new(
            body("Mercury"),
            TimestampMicros::new(100),
            Some(vec![phase("R2", 110), phase("R3", 120), phase("D4", 130)]),
        )
        .unwrap();
        assert_eq!(three.phases().unwrap()[1].name(), "R3");
        assert_eq!(three.phases().unwrap()[1].at().as_i64(), 120);

        for invalid in [
            vec![phase("first", 100)],
            vec![phase("first", 99)],
            vec![phase("first", 110), phase("second", 110)],
            vec![phase("first", 120), phase("second", 110)],
        ] {
            assert_eq!(
                SynodEvent::new(body("Moon"), TimestampMicros::new(100), Some(invalid)),
                Err(ModelError::InvalidPhaseOrder)
            );
        }
        assert_eq!(
            SynodPhase::new(" ", TimestampMicros::new(1)),
            Err(ModelError::InvalidPhaseName)
        );
    }

    #[test]
    fn motion_ingress_and_unified_event_retain_body_and_instant() {
        let sign = ZodiacSign::try_from(4_i32).unwrap();
        let motion = MotionEvent::new(
            body("Mars"),
            TimestampMicros::new(-5),
            MotionDirection::Retrograde,
            sign,
        );
        assert_eq!(motion.direction(), MotionDirection::Retrograde);
        assert_eq!(motion.sign().index(), 4);
        let ingress = IngressEvent::new(body("Moon"), TimestampMicros::new(10), sign);
        assert_eq!(ingress.sign().index(), 4);
        let events = [Event::Motion(motion), Event::Ingress(ingress)];
        assert_eq!(events[0].body().as_str(), "Mars");
        assert_eq!(events[0].at().as_i64(), -5);
        assert_eq!(events[1].body().as_str(), "Moon");
        assert_eq!(events[1].at().as_i64(), 10);
    }

    #[test]
    fn reexports_format_probe_without_claiming_binary_support() {
        assert!(probe_magic(&VSEPH_MAGIC).is_ok());
        assert!(probe_magic(b"XXXXX").is_err());
    }
}

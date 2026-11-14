/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Parse untrusted VSEPH-P0 JSON metadata without discarding duplicate keys.
//!
//! Deserializing directly into serde_json::Value silently keeps one value for
//! duplicate object members, including names written with Unicode escapes.
//! The independent Python P0 auditor refuses such headers: the native reader
//! must make the same fail-closed decision before trusting any index offsets.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};
use std::fmt;

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with no duplicate object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(Number::from(value))))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(Number::from(value))))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        let number =
            Number::from_f64(value).ok_or_else(|| E::custom("non-finite JSON number"))?;
        Ok(UniqueValue(Value::Number(number)))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        UniqueValue::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON object key: {key:?}")));
            }
            let value = access.next_value::<UniqueValue>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

/// Retain serde_json's UTF-8, numeric and recursion checks, while rejecting
/// ambiguous object members at any depth (not just the outer header object).
pub(crate) fn parse_header(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<UniqueValue>(bytes).map(|parsed| parsed.0)
}

#[cfg(test)]
mod tests {
    use super::parse_header;

    #[test]
    fn rejects_duplicate_keys_at_any_depth() {
        for payload in [
            r#"{"format":"P0","format":"P0"}"#,
            r#"{"sections":{"synod":1,"synod":1}}"#,
            r#"{"toc":[{"body":"Moon","body":"Moon"}]}"#,
            r#"{"sections":{},"\u0073ections":{}}"#,
        ] {
            let error = parse_header(payload.as_bytes()).unwrap_err();
            assert!(error.to_string().contains("duplicate JSON object key"), "{error}");
        }
    }

    #[test]
    fn keeps_distinct_numbers_null_and_arrays_without_coercion() {
        let json = r#"{"sections":{"synod":2},"flags":[true,null,1,1.5]}"#;
        let parsed = parse_header(json.as_bytes()).unwrap();
        assert_eq!(parsed["sections"]["synod"], 2);
        assert_eq!(parsed["flags"][0], true);
        assert!(parsed["flags"][1].is_null());
        assert_eq!(parsed["flags"][2].as_i64(), Some(1));
        assert_eq!(parsed["flags"][3].as_f64(), Some(1.5));
    }

    #[test]
    fn retains_standard_json_syntax_and_depth_checks() {
        for payload in [
            &b"{\"a\":NaN}"[..],
            &b"{\"a\":1e400}"[..],
            &b"{\"a\":1} garbage"[..],
            &b"{\"a\":\xff}"[..],
        ] {
            assert!(parse_header(payload).is_err());
        }
    }
}

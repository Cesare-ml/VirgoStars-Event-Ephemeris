# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Source-only regressions for the Rust/Python JSON transport and Moon alias."""
import unittest

from virgostars_events._wire import decode_native_json, same_json_value


class NativeJsonUnicodeTests(unittest.TestCase):
    def test_valid_bmp_and_paired_supplementary_scalars_are_preserved(self):
        self.assertEqual(
            decode_native_json('{"\\u00e8\\ud83d\\ude00":"\\ud83d\\ude03"}'),
            {"è😀": "😃"},
        )

    def test_lone_high_surrogate_in_nested_value_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unpaired Unicode surrogate"):
            decode_native_json('{"synods":{"Moon":["\\ud800"]}}')

    def test_lone_low_surrogate_in_nested_key_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unpaired Unicode surrogate"):
            decode_native_json('{"synods":{"\\udfff":42}}')

    def test_unescaped_surrogate_in_python_text_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unpaired Unicode surrogate"):
            decode_native_json('{"body":"' + chr(0xD800) + '"}')

    def test_existing_duplicate_and_nonfinite_rejections_remain(self):
        for payload in ('{"id":1,"\\u0069d":1}', '{"id":NaN}', '{"id":1e400}'):
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                decode_native_json(payload)

    def test_json_integer_bounds_match_rust_i64_u64(self):
        self.assertEqual(
            decode_native_json('{"min":-9223372036854775808,"max":18446744073709551615}'),
            {"min": -(1 << 63), "max": (1 << 64) - 1},
        )
        for literal in ("-9223372036854775809", "18446744073709551616",
                        "999999999999999999999999999999"):
            with self.subTest(value=literal), self.assertRaisesRegex(
                ValueError, "outside Rust i64/u64 range"
            ):
                decode_native_json('{"id":' + literal + '}')

    def test_alias_comparison_is_type_exact_in_nested_containers(self):
        matching = {"Moon": [{"source_id": 11, "extras": [None, 3.0, True]}]}
        self.assertTrue(same_json_value(matching, matching))
        self.assertFalse(same_json_value({"v": [1]}, {"v": [True]}))
        self.assertFalse(same_json_value({"v": [1]}, {"v": [1.0]}))
        self.assertFalse(same_json_value({"v": [None]}, {"v": [[]]}))
        self.assertFalse(same_json_value({"x": 1}, {"y": 1}))
        self.assertTrue(same_json_value({"a": 1, "b": 2}, {"b": 2, "a": 1}))

    def test_alias_comparison_supports_deep_equal_json_without_recursion(self):
        left, right = 1, 1
        for _ in range(700):
            left = [left]
            right = [right]
        self.assertTrue(same_json_value(left, right))

    def test_alias_comparison_detects_deep_leaf_mismatch(self):
        left, right = 1, 2
        for _ in range(700):
            left = {"v": left}
            right = {"v": right}
        self.assertFalse(same_json_value(left, right))


if __name__ == "__main__":
    unittest.main()

# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Source-only regressions for the Rust/Python JSON Unicode boundary."""
import unittest

from virgostars_events._wire import decode_native_json


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


if __name__ == "__main__":
    unittest.main()

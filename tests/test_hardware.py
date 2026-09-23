"""
Unit tests for hardware acceleration and GPU detection in steM.
"""

import unittest
from stem.core.hardware import (
    GpuInfo,
    get_hardware_info,
    get_optimal_device,
    get_recommended_segment_size,
)


class TestHardware(unittest.TestCase):
    def test_hardware_info_structure(self):
        info = get_hardware_info()
        self.assertIsInstance(info, GpuInfo)
        self.assertIsInstance(info.available, bool)
        self.assertIsInstance(info.name, str)
        self.assertIsInstance(info.vram_total_mb, int)
        self.assertIsInstance(info.vram_free_mb, int)
        self.assertIsInstance(info.is_low_vram, bool)

    def test_optimal_device_cpu_preference(self):
        self.assertEqual(get_optimal_device(preference="cpu"), "cpu")

    def test_optimal_device_auto(self):
        device = get_optimal_device(preference="auto")
        self.assertIn(device, ("cuda", "cpu"))

    def test_recommended_segment_size(self):
        self.assertLessEqual(get_recommended_segment_size(is_low_vram=True), 6)
        self.assertGreater(get_recommended_segment_size(is_low_vram=False), 6)


if __name__ == "__main__":
    unittest.main()

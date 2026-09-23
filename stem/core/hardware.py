"""
steM. - Hardware Acceleration & GPU Environment Detection
Inspects NVIDIA CUDA, VRAM capacity, and provides memory-conscious defaults.
"""

import shutil
import subprocess
from dataclasses import dataclass
from typing import Optional, Tuple


@dataclass
class GpuInfo:
    available: bool
    name: str
    vram_total_mb: int
    vram_free_mb: int
    driver_version: str
    cuda_available: bool
    torch_cuda_version: Optional[str] = None
    is_low_vram: bool = False  # <= 4096 MB VRAM


def detect_via_nvidia_smi() -> Optional[Tuple[str, int, int, str]]:
    """Inspect GPU hardware using nvidia-smi if available."""
    if not shutil.which("nvidia-smi"):
        return None
    try:
        cmd = [
            "nvidia-smi",
            "--query-gpu=name,memory.total,memory.free,driver_version",
            "--format=csv,noheader,nounits",
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=3)
        line = result.stdout.strip().split("\n")[0]
        parts = [p.strip() for p in line.split(",")]
        if len(parts) >= 4:
            name = parts[0]
            total_mb = int(float(parts[1]))
            free_mb = int(float(parts[2]))
            driver = parts[3]
            return name, total_mb, free_mb, driver
    except Exception:
        pass
    return None


def get_hardware_info() -> GpuInfo:
    """Returns comprehensive GPU and CUDA hardware information."""
    smi_data = detect_via_nvidia_smi()
    torch_cuda_available = False
    torch_cuda_version = None
    torch_device_name = ""
    torch_total_mb = 0

    try:
        import torch
        torch_cuda_available = torch.cuda.is_available()
        if torch_cuda_available:
            torch_cuda_version = torch.version.cuda
            torch_device_name = torch.cuda.get_device_name(0)
            props = torch.cuda.get_device_properties(0)
            torch_total_mb = int(props.total_memory / (1024 * 1024))
    except Exception:
        pass

    if smi_data:
        smi_name, smi_total, smi_free, smi_driver = smi_data
        name = torch_device_name or smi_name
        total_mb = torch_total_mb if torch_total_mb > 0 else smi_total
        cuda_avail = torch_cuda_available or (smi_total > 0)
        is_low = total_mb <= 4200  # Catches 4096 MiB RTX 3050 Laptop GPU
        return GpuInfo(
            available=True,
            name=name,
            vram_total_mb=total_mb,
            vram_free_mb=smi_free,
            driver_version=smi_driver,
            cuda_available=cuda_avail,
            torch_cuda_version=torch_cuda_version,
            is_low_vram=is_low,
        )

    return GpuInfo(
        available=False,
        name="No NVIDIA GPU Detected (CPU Fallback)",
        vram_total_mb=0,
        vram_free_mb=0,
        driver_version="N/A",
        cuda_available=False,
        torch_cuda_version=None,
        is_low_vram=True,
    )


def get_optimal_device(preference: str = "auto") -> str:
    """
    Determines whether 'cuda' or 'cpu' should be used.
    preference: 'auto', 'cuda', or 'cpu'
    """
    if preference == "cpu":
        return "cpu"
    info = get_hardware_info()
    if preference == "cuda":
        return "cuda" if info.cuda_available else "cpu"
    # Auto mode
    return "cuda" if info.cuda_available else "cpu"


def get_recommended_segment_size(is_low_vram: bool = True) -> int:
    """
    Recommended segment size (seconds) for Demucs separation.
    Demucs default is ~7.8s. For 4GB VRAM (RTX 3050), 4-6s prevents OOM.
    """
    return 6 if is_low_vram else 10

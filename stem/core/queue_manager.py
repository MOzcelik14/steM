"""
steM. - Processing Queue Manager
Coordinates asynchronous multi-track batch separation with Demucs.
"""

import threading
import uuid
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Callable, Dict, List, Optional

import gi
gi.require_version("GLib", "2.0")
from gi.repository import GLib

from stem.core.audio_metadata import AudioFileInfo
from stem.core.demucs_runner import DemucsRunner, SeparationTask


class QueueStatus(Enum):
    PENDING = "pending"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class QueueItem:
    item_id: str
    file_info: AudioFileInfo
    task: SeparationTask
    status: QueueStatus = QueueStatus.PENDING
    progress: float = 0.0
    status_text: str = "Pending"
    stems: Dict[str, Path] = field(default_factory=dict)
    error_message: Optional[str] = None


class QueueManager:
    """Manages an orderly queue of separation jobs."""

    def __init__(self) -> None:
        self.items: List[QueueItem] = []
        self._runner = DemucsRunner()
        self._is_running: bool = False
        self._current_item: Optional[QueueItem] = None
        self._lock = threading.Lock()

        # UI callbacks (dispatched via GLib.idle_add)
        self.on_item_status_changed: Optional[Callable[[QueueItem], None]] = None
        self.on_queue_completed: Optional[Callable[[], None]] = None
        self.on_oom_detected: Optional[Callable[[QueueItem, str], None]] = None

    @property
    def is_busy(self) -> bool:
        return self._is_running

    def add_item(self, file_info: AudioFileInfo, task: SeparationTask) -> QueueItem:
        """Adds a track to the separation queue."""
        item = QueueItem(
            item_id=str(uuid.uuid4())[:8],
            file_info=file_info,
            task=task,
        )
        with self._lock:
            self.items.append(item)
        self._notify_item(item)
        return item

    def remove_item(self, item_id: str) -> bool:
        with self._lock:
            for i, it in enumerate(self.items):
                if it.item_id == item_id:
                    if it == self._current_item:
                        self.cancel_current()
                    self.items.pop(i)
                    return True
        return False

    def clear(self) -> None:
        self.cancel_current()
        with self._lock:
            self.items.clear()

    def start_queue(self) -> None:
        """Begins processing queue items sequentially."""
        if self._is_running:
            return
        self._is_running = True
        threading.Thread(target=self._process_loop, daemon=True).start()

    def cancel_current(self) -> None:
        """Cancels the currently running separation job."""
        if self._runner.is_running:
            self._runner.cancel()
        if self._current_item:
            self._current_item.status = QueueStatus.CANCELLED
            self._current_item.status_text = "Cancelled"
            self._notify_item(self._current_item)

    def _process_loop(self) -> None:
        while self._is_running:
            item_to_process = None
            with self._lock:
                for it in self.items:
                    if it.status == QueueStatus.PENDING:
                        item_to_process = it
                        break

            if not item_to_process:
                break

            self._current_item = item_to_process
            item_to_process.status = QueueStatus.PROCESSING
            item_to_process.progress = 0.05
            item_to_process.status_text = "Starting..."
            self._notify_item(item_to_process)

            separation_event = threading.Event()

            def _on_progress(pct: float, msg: str):
                item_to_process.progress = pct
                item_to_process.status_text = msg
                self._notify_item(item_to_process)

            def _on_finished(success: bool, err: Optional[str], stems: Dict[str, Path]):
                if success:
                    item_to_process.status = QueueStatus.COMPLETED
                    item_to_process.progress = 1.0
                    item_to_process.status_text = "Completed"
                    item_to_process.stems = stems
                else:
                    if item_to_process.status != QueueStatus.CANCELLED:
                        item_to_process.status = QueueStatus.FAILED
                        item_to_process.status_text = "Failed"
                    item_to_process.error_message = err
                self._notify_item(item_to_process)
                separation_event.set()

            def _on_oom(msg: str):
                if self.on_oom_detected:
                    GLib.idle_add(self.on_oom_detected, item_to_process, msg)

            self._runner.run_separation_async(
                task=item_to_process.task,
                on_progress=_on_progress,
                on_finished=_on_finished,
                on_oom=_on_oom,
            )

            separation_event.wait()
            self._current_item = None

        self._is_running = False
        if self.on_queue_completed:
            GLib.idle_add(self.on_queue_completed)

    def _notify_item(self, item: QueueItem) -> None:
        if self.on_item_status_changed:
            GLib.idle_add(self.on_item_status_changed, item)

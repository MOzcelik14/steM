"""
steM. - Separation View
Queue management, model selection, hardware device options, and live separation progress.
"""

from pathlib import Path
from typing import Callable, Dict, List, Optional
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, GLib, Gtk

from stem.config import AVAILABLE_MODELS, DEFAULT_SETTINGS, settings
from stem.core.audio_metadata import AudioFileInfo
from stem.core.demucs_runner import SeparationTask
from stem.core.hardware import get_hardware_info, get_optimal_device
from stem.core.queue_manager import QueueItem, QueueManager, QueueStatus


class QueueItemRow(Adw.ActionRow):
    """Visual row for an individual audio track in the separation queue."""

    def __init__(
        self,
        item: QueueItem,
        on_remove: Callable[[str], None],
        on_open_mixer: Callable[[QueueItem], None],
    ) -> None:
        super().__init__()
        self.item = item
        self.on_remove = on_remove
        self.on_open_mixer = on_open_mixer

        self.set_title(item.file_info.filename)
        self.set_subtitle(
            f"{item.file_info.duration_str} • {item.file_info.format_name} • "
            f"{item.file_info.sample_rate} Hz • {item.file_info.channels_str} • "
            f"{item.file_info.file_size_mb} MB"
        )

        # Right-side status and action container
        self.status_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.status_box.set_valign(Gtk.Align.CENTER)

        # Progress bar
        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_size_request(140, -1)
        self.progress_bar.set_fraction(item.progress)
        self.progress_bar.set_visible(item.status == QueueStatus.PROCESSING)
        self.status_box.append(self.progress_bar)

        # Status Label
        self.status_label = Gtk.Label(label=item.status_text)
        self.status_label.set_css_classes(["dim-label"])
        self.status_box.append(self.status_label)

        # Open in Mixer button (visible on completion)
        self.mixer_btn = Gtk.Button.new_from_icon_name("media-playback-start-symbolic")
        self.mixer_btn.set_tooltip_text("Open in Mixer Studio")
        self.mixer_btn.set_css_classes(["flat", "circular"])
        self.mixer_btn.set_visible(item.status == QueueStatus.COMPLETED)
        self.mixer_btn.connect("clicked", lambda b: self.on_open_mixer(self.item))
        self.status_box.append(self.mixer_btn)

        # Remove button
        self.remove_btn = Gtk.Button.new_from_icon_name("user-trash-symbolic")
        self.remove_btn.set_tooltip_text("Remove from Queue")
        self.remove_btn.set_css_classes(["flat", "circular"])
        self.remove_btn.connect("clicked", lambda b: self.on_remove(self.item.item_id))
        self.status_box.append(self.remove_btn)

        self.add_suffix(self.status_box)

    def update_state(self) -> None:
        self.progress_bar.set_fraction(self.item.progress)
        self.progress_bar.set_visible(self.item.status == QueueStatus.PROCESSING)
        self.status_label.set_label(self.item.status_text)
        self.mixer_btn.set_visible(self.item.status == QueueStatus.COMPLETED)


class SeparationView(Gtk.Box):
    """Main view for configuring AI separation settings and running the queue."""

    def __init__(
        self,
        queue_manager: QueueManager,
        on_add_files_requested: Callable[[], None],
        on_open_mixer: Callable[[QueueItem], None],
    ) -> None:
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        self.queue_manager = queue_manager
        self.on_add_files_requested = on_add_files_requested
        self.on_open_mixer = on_open_mixer

        self.set_margin_top(16)
        self.set_margin_bottom(16)
        self.set_margin_start(20)
        self.set_margin_end(20)

        self._row_map: Dict[str, QueueItemRow] = {}
        self._build_ui()

        # Connect queue events
        self.queue_manager.on_item_status_changed = self._on_item_status_changed
        self.queue_manager.on_queue_completed = self._on_queue_completed

    def _build_ui(self) -> None:
        # Top Paned/Split layout: Left is Queue list, Right is Separation Settings
        paned = Gtk.Paned(orientation=Gtk.Orientation.HORIZONTAL)
        paned.set_vexpand(True)
        paned.set_hexpand(True)
        paned.set_position(480)

        # LEFT: Queue Container
        left_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        left_box.set_margin_end(10)

        queue_header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        q_title = Gtk.Label(label="Audio Processing Queue")
        q_title.set_css_classes(["heading"])
        q_title.set_hexpand(True)
        q_title.set_halign(Gtk.Align.START)
        queue_header.append(q_title)

        add_btn = Gtk.Button(label="Add Files...")
        add_btn.set_css_classes(["suggested-action", "pill"])
        add_btn.connect("clicked", lambda b: self.on_add_files_requested())
        queue_header.append(add_btn)

        clear_btn = Gtk.Button(label="Clear")
        clear_btn.set_css_classes(["flat", "pill"])
        clear_btn.connect("clicked", self._on_clear_queue)
        queue_header.append(clear_btn)

        left_box.append(queue_header)

        # Queue List Box inside ScrolledWindow
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_hexpand(True)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self.queue_list = Gtk.ListBox()
        self.queue_list.set_selection_mode(Gtk.SelectionMode.NONE)
        self.queue_list.set_css_classes(["boxed-list"])
        scrolled.set_child(self.queue_list)

        left_box.append(scrolled)
        paned.set_start_child(left_box)

        # RIGHT: Settings & Controls Card
        right_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        right_box.set_margin_start(10)
        right_box.set_size_request(340, -1)

        pref_group = Adw.PreferencesGroup()
        pref_group.set_title("AI Separation Settings")
        pref_group.set_description("Demucs Model &amp; Hardware Configuration")

        # Model selection row
        self.model_row = Adw.ComboRow()
        self.model_row.set_title("Demucs Model")
        model_names = Gtk.StringList.new([
            "HTDemucs v4 (Default 4-Stem)",
            "HTDemucs Fine-Tuned (Studio Quality)",
            "HTDemucs 6-Stems (with Guitar/Piano)",
            "MDX-Net Extra",
        ])
        self.model_row.set_model(model_names)
        self.model_keys = ["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"]
        self.model_row.set_selected(0)
        pref_group.add(self.model_row)

        # Two-stems toggle row (Vocals vs Instrumental)
        self.two_stems_row = Adw.SwitchRow()
        self.two_stems_row.set_title("2-Stem Isolation (Vocal + Instrumental)")
        self.two_stems_row.set_subtitle("Extract only vocals and backing track")
        self.two_stems_row.set_active(settings.get("two_stems", False))
        pref_group.add(self.two_stems_row)

        # Device selection row
        self.device_row = Adw.ComboRow()
        self.device_row.set_title("Processing Device")
        hw = get_hardware_info()
        dev_options = [
            f"Auto-Detect ({'CUDA GPU' if hw.cuda_available else 'CPU'})",
            f"NVIDIA CUDA ({hw.name})",
            "CPU Fallback (Multithreaded)",
        ]
        self.device_row.set_model(Gtk.StringList.new(dev_options))
        self.device_row.set_selected(0)
        pref_group.add(self.device_row)

        # Quality / Shifts row
        self.quality_row = Adw.ComboRow()
        self.quality_row.set_title("Separation Precision (Shifts)")
        self.quality_row.set_model(Gtk.StringList.new([
            "Fast (Shifts: 0)",
            "Balanced Studio (Shifts: 1 — Recommended)",
            "High Fidelity (Shifts: 2)",
        ]))
        self.quality_row.set_selected(1)
        pref_group.add(self.quality_row)

        # Low VRAM Optimization Switch (for 4GB RTX 3050)
        self.vram_row = Adw.SwitchRow()
        self.vram_row.set_title("Low VRAM Safety (4 GB Optimization)")
        self.vram_row.set_subtitle("Applies segment chunking to prevent CUDA OOM")
        self.vram_row.set_active(True)
        pref_group.add(self.vram_row)

        # Output format row
        self.format_row = Adw.ComboRow()
        self.format_row.set_title("Export Format")
        self.format_row.set_model(Gtk.StringList.new(["WAV (24-bit Lossless)", "MP3 (320 kbps High Quality)"]))
        self.format_row.set_selected(0)
        pref_group.add(self.format_row)

        # Output directory action row
        self.out_dir_row = Adw.ActionRow()
        self.out_dir_row.set_title("Destination Directory")
        self.current_out_dir = Path(settings.get("output_dir"))
        self.out_dir_row.set_subtitle(str(self.current_out_dir))
        dir_btn = Gtk.Button.new_from_icon_name("folder-open-symbolic")
        dir_btn.set_valign(Gtk.Align.CENTER)
        dir_btn.connect("clicked", self._on_choose_out_dir)
        self.out_dir_row.add_suffix(dir_btn)
        pref_group.add(self.out_dir_row)

        right_box.append(pref_group)

        # Bottom Action Buttons
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        actions_box.set_halign(Gtk.Align.FILL)

        self.start_btn = Gtk.Button(label="Separate Stems")
        self.start_btn.set_hexpand(True)
        self.start_btn.set_css_classes(["accent-button", "pill"])
        self.start_btn.connect("clicked", self._on_start_separation)
        actions_box.append(self.start_btn)

        self.cancel_btn = Gtk.Button(label="Cancel")
        self.cancel_btn.set_css_classes(["destructive-action", "pill"])
        self.cancel_btn.set_sensitive(False)
        self.cancel_btn.connect("clicked", self._on_cancel_separation)
        actions_box.append(self.cancel_btn)

        right_box.append(actions_box)
        paned.set_end_child(right_box)

        self.append(paned)

    def add_track_to_queue(self, file_info: AudioFileInfo) -> None:
        """Constructs a SeparationTask and adds track to queue."""
        selected_idx = self.model_row.get_selected()
        model_name = self.model_keys[selected_idx] if selected_idx < len(self.model_keys) else "htdemucs"
        two_stems = "vocals" if self.two_stems_row.get_active() else None

        dev_idx = self.device_row.get_selected()
        if dev_idx == 1:
            device = "cuda"
        elif dev_idx == 2:
            device = "cpu"
        else:
            device = get_optimal_device("auto")

        is_low_vram = self.vram_row.get_active()
        segment = 6 if is_low_vram else 10
        shifts = self.quality_row.get_selected()
        fmt = "wav" if self.format_row.get_selected() == 0 else "mp3"

        task = SeparationTask(
            audio_path=file_info.path,
            output_dir=self.current_out_dir,
            model_name=model_name,
            two_stems=two_stems,
            device=device,
            segment_size=segment,
            shifts=shifts,
            overlap=0.25,
            export_format=fmt,
            mp3_bitrate=320,
        )

        item = self.queue_manager.add_item(file_info, task)
        row = QueueItemRow(item, self._on_remove_item, self.on_open_mixer)
        self._row_map[item.item_id] = row
        self.queue_list.append(row)

    def _on_remove_item(self, item_id: str) -> None:
        if self.queue_manager.remove_item(item_id):
            row = self._row_map.pop(item_id, None)
            if row:
                self.queue_list.remove(row)

    def _on_clear_queue(self, btn) -> None:
        self.queue_manager.clear()
        for row in self._row_map.values():
            self.queue_list.remove(row)
        self._row_map.clear()

    def _on_start_separation(self, btn) -> None:
        if not self.queue_manager.items:
            return
        self.start_btn.set_sensitive(False)
        self.cancel_btn.set_sensitive(True)
        self.queue_manager.start_queue()

    def _on_cancel_separation(self, btn) -> None:
        self.queue_manager.cancel_current()
        self.cancel_btn.set_sensitive(False)
        self.start_btn.set_sensitive(True)

    def _on_item_status_changed(self, item: QueueItem) -> None:
        if item.item_id in self._row_map:
            self._row_map[item.item_id].update_state()

    def _on_queue_completed(self) -> None:
        self.start_btn.set_sensitive(True)
        self.cancel_btn.set_sensitive(False)

    def _on_choose_out_dir(self, btn) -> None:
        dialog = Gtk.FileDialog.new()
        dialog.set_title("Select Output Folder — steM.")
        root = self.get_root()
        dialog.select_folder(root, None, self._on_out_dir_selected)

    def _on_out_dir_selected(self, dialog, result) -> None:
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                p = Path(folder.get_path())
                self.current_out_dir = p
                self.out_dir_row.set_subtitle(str(p))
                settings.set("output_dir", str(p))
        except Exception:
            pass

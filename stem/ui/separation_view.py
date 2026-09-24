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
from stem.i18n import add_language_listener, t


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
        self.status_label = Gtk.Label()
        self.status_label.set_css_classes(["dim-label"])
        self.status_box.append(self.status_label)

        # Open in Mixer button (visible on completion)
        self.mixer_btn = Gtk.Button.new_from_icon_name("media-playback-start-symbolic")
        self.mixer_btn.set_tooltip_text(t("tooltip_open_mixer"))
        self.mixer_btn.set_css_classes(["flat", "circular"])
        self.mixer_btn.set_visible(item.status == QueueStatus.COMPLETED)
        self.mixer_btn.connect("clicked", lambda b: self.on_open_mixer(self.item))
        self.status_box.append(self.mixer_btn)

        # Remove button
        self.remove_btn = Gtk.Button.new_from_icon_name("user-trash-symbolic")
        self.remove_btn.set_tooltip_text(t("tooltip_remove_queue"))
        self.remove_btn.set_css_classes(["flat", "circular"])
        self.remove_btn.connect("clicked", lambda b: self.on_remove(self.item.item_id))
        self.status_box.append(self.remove_btn)

        self.add_suffix(self.status_box)
        self.update_state()

    def update_state(self) -> None:
        self.progress_bar.set_fraction(self.item.progress)
        self.progress_bar.set_visible(self.item.status == QueueStatus.PROCESSING)
        if self.item.status == QueueStatus.PROCESSING:
            self.status_label.set_label(t("status_processing", pct=int(self.item.progress * 100)))
        elif self.item.status == QueueStatus.COMPLETED:
            self.status_label.set_label(t("status_completed"))
        elif self.item.status == QueueStatus.CANCELLED:
            self.status_label.set_label(t("status_cancelled"))
        elif self.item.status == QueueStatus.FAILED:
            self.status_label.set_label(t("status_failed"))
        else:
            self.status_label.set_label(t("status_pending"))
        self.mixer_btn.set_visible(self.item.status == QueueStatus.COMPLETED)

    def update_locale(self) -> None:
        self.mixer_btn.set_tooltip_text(t("tooltip_open_mixer"))
        self.remove_btn.set_tooltip_text(t("tooltip_remove_queue"))
        self.update_state()


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
        add_language_listener(self.update_locale)

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
        self.q_title = Gtk.Label(label=t("queue_title"))
        self.q_title.set_css_classes(["heading"])
        self.q_title.set_hexpand(True)
        self.q_title.set_halign(Gtk.Align.START)
        queue_header.append(self.q_title)

        self.add_btn = Gtk.Button(label=t("add_files"))
        self.add_btn.set_css_classes(["suggested-action", "pill"])
        self.add_btn.connect("clicked", lambda b: self.on_add_files_requested())
        queue_header.append(self.add_btn)

        self.clear_btn = Gtk.Button(label=t("clear"))
        self.clear_btn.set_css_classes(["flat", "pill"])
        self.clear_btn.connect("clicked", self._on_clear_queue)
        queue_header.append(self.clear_btn)

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

        self.pref_group = Adw.PreferencesGroup()

        # Model selection row
        self.model_row = Adw.ComboRow()
        self.model_keys = ["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"]
        self.pref_group.add(self.model_row)

        # Two-stems toggle row (Vocals vs Instrumental)
        self.two_stems_row = Adw.SwitchRow()
        self.two_stems_row.set_active(settings.get("two_stems", False))
        self.pref_group.add(self.two_stems_row)

        # Device selection row
        self.device_row = Adw.ComboRow()
        self.pref_group.add(self.device_row)

        # Quality / Shifts row
        self.quality_row = Adw.ComboRow()
        self.pref_group.add(self.quality_row)

        # Low VRAM Optimization Switch (for 4GB RTX 3050)
        self.vram_row = Adw.SwitchRow()
        self.vram_row.set_active(True)
        self.pref_group.add(self.vram_row)

        # Output format row
        self.format_row = Adw.ComboRow()
        self.pref_group.add(self.format_row)

        # Output directory action row
        self.out_dir_row = Adw.ActionRow()
        self.current_out_dir = Path(settings.get("output_dir"))
        self.out_dir_row.set_subtitle(str(self.current_out_dir))
        dir_btn = Gtk.Button.new_from_icon_name("folder-open-symbolic")
        dir_btn.set_valign(Gtk.Align.CENTER)
        dir_btn.connect("clicked", self._on_choose_out_dir)
        self.out_dir_row.add_suffix(dir_btn)
        self.pref_group.add(self.out_dir_row)

        right_box.append(self.pref_group)

        # Bottom Action Buttons
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        actions_box.set_halign(Gtk.Align.FILL)

        self.start_btn = Gtk.Button()
        self.start_btn.set_hexpand(True)
        self.start_btn.set_css_classes(["accent-button", "pill"])
        self.start_btn.connect("clicked", self._on_start_separation)
        actions_box.append(self.start_btn)

        self.cancel_btn = Gtk.Button()
        self.cancel_btn.set_css_classes(["destructive-action", "pill"])
        self.cancel_btn.set_sensitive(False)
        self.cancel_btn.connect("clicked", self._on_cancel_separation)
        actions_box.append(self.cancel_btn)

        right_box.append(actions_box)
        paned.set_end_child(right_box)

        # Apply initial localized strings
        self.update_locale()
        self.model_row.set_selected(0)
        self.device_row.set_selected(0)
        self.quality_row.set_selected(1)
        self.format_row.set_selected(0)

        self.append(paned)

    def update_locale(self, lang: Optional[str] = None) -> None:
        """Updates all text in SeparationView to reflect active language."""
        self.q_title.set_label(t("queue_title"))
        self.add_btn.set_label(t("add_files"))
        self.clear_btn.set_label(t("clear"))
        self.pref_group.set_title(t("settings_group_title"))
        self.pref_group.set_description(t("settings_group_desc"))

        # Model row
        curr_m = self.model_row.get_selected()
        self.model_row.set_title(t("demucs_model"))
        self.model_row.set_model(Gtk.StringList.new([
            t("model_htdemucs"),
            t("model_htdemucs_ft"),
            t("model_htdemucs_6s"),
            t("model_mdx_extra"),
        ]))
        self.model_row.set_selected(curr_m if curr_m != 4294967295 else 0)

        # Two-stems row
        self.two_stems_row.set_title(t("two_stems_title"))
        self.two_stems_row.set_subtitle(t("two_stems_sub"))

        # Device row
        hw = get_hardware_info()
        curr_dev = self.device_row.get_selected()
        self.device_row.set_title(t("device_title"))
        dev_label = "CUDA GPU" if hw.cuda_available else "CPU"
        dev_cuda_str = t("device_cuda", name=hw.name) if hw.cuda_available else "NVIDIA CUDA (N/A)"
        self.device_row.set_model(Gtk.StringList.new([
            t("device_auto", dev=dev_label),
            dev_cuda_str,
            t("device_cpu"),
        ]))
        self.device_row.set_selected(curr_dev if curr_dev != 4294967295 else 0)

        # Quality row
        curr_q = self.quality_row.get_selected()
        self.quality_row.set_title(t("quality_title"))
        self.quality_row.set_model(Gtk.StringList.new([
            t("quality_fast"),
            t("quality_balanced"),
            t("quality_high"),
        ]))
        self.quality_row.set_selected(curr_q if curr_q != 4294967295 else 1)

        # VRAM row
        self.vram_row.set_title(t("low_vram_title"))
        self.vram_row.set_subtitle(t("low_vram_sub"))

        # Export format row
        curr_f = self.format_row.get_selected()
        self.format_row.set_title(t("export_format_title"))
        self.format_row.set_model(Gtk.StringList.new([
            t("format_wav"),
            t("format_mp3"),
        ]))
        self.format_row.set_selected(curr_f if curr_f != 4294967295 else 0)

        # Out dir row
        self.out_dir_row.set_title(t("dest_dir_title"))

        # Buttons
        self.start_btn.set_label(t("btn_separate"))
        self.cancel_btn.set_label(t("btn_cancel"))

        # Update items in queue
        for row in self._row_map.values():
            row.update_locale()

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
        dialog.set_title(t("select_output_folder"))
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

"""
steM. - About Dialog
Presents application branding, developer attribution, and system details.
"""

import sys
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from stem.config import (
    APP_AUTHOR,
    APP_ID,
    APP_NAME,
    APP_SUBTITLE,
    APP_VERSION,
    GITHUB_URL,
)
from stem.core.hardware import get_hardware_info
from stem.i18n import t


def show_about_dialog(parent_window: Gtk.Window) -> None:
    """Displays the Libadwaita About dialog with steM. branding."""
    hw = get_hardware_info()
    gpu_desc = f"{hw.name} ({hw.vram_total_mb} MB VRAM)" if hw.cuda_available else "CPU"
    comments = f"{t('app_subtitle')}\n\n" + t(
        "about_comments",
        py_ver=sys.version.split()[0],
        gpu_info=gpu_desc,
    )

    dialog = Adw.AboutDialog()
    dialog.set_application_name(APP_NAME)
    dialog.set_application_icon(APP_ID)
    dialog.set_developer_name(APP_AUTHOR)
    dialog.set_version(APP_VERSION)
    dialog.set_comments(comments)
    dialog.set_website(GITHUB_URL)
    dialog.set_issue_url(f"{GITHUB_URL}/issues")
    dialog.set_license_type(Gtk.License.MIT_X11)
    dialog.set_copyright(f"© 2026 {APP_AUTHOR}")

    dialog.present(parent_window)

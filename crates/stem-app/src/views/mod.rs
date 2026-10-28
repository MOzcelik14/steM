pub mod dialogs;
pub mod effects;
pub mod menu;
pub mod mixer;
pub mod spectrogram;
pub mod timeline;
pub mod transport;

pub use dialogs::render_dialogs;
pub use effects::{render_effects_dialog, EffectsRackState};
pub use menu::render_menu_bar;
pub use mixer::render_mixer_panel;
pub use spectrogram::{render_spectrogram_panel, SpectrogramState};
pub use timeline::render_timeline_view;
pub use transport::render_transport_bar;

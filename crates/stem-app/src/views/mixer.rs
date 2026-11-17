//! Mixer panel view with vertical channel strips and master bus fader

use crate::app::StemApp;
use egui::{Color32, Vec2};
use stem_i18n::format_db;

pub fn render_mixer_panel(app: &mut StemApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

        let strip_width = 80.0;
        let num_tracks = app.project.tracks.len();

        // ---------------- Individual Track Strips ----------------
        for t_idx in 0..num_tracks {
            let track = &app.project.tracks[t_idx];
            let track_name = track.name.clone();

            ui.group(|ui| {
                ui.set_width(strip_width);
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&track_name).strong()).truncate());

                    // Mute / Solo buttons
                    ui.horizontal(|ui| {
                        let mut muted = app.project.tracks[t_idx].muted;
                        let mute_color = if muted {
                            Color32::from_rgb(230, 80, 80)
                        } else {
                            Color32::GRAY
                        };
                        if ui
                            .selectable_label(
                                muted,
                                egui::RichText::new(app.i18n.t("track-header-mute"))
                                    .color(mute_color),
                            )
                            .clicked()
                        {
                            muted = !muted;
                            app.action_set_track_mute(t_idx, muted);
                        }

                        let mut soloed = app.project.tracks[t_idx].soloed;
                        let solo_color = if soloed {
                            Color32::from_rgb(220, 200, 50)
                        } else {
                            Color32::GRAY
                        };
                        if ui
                            .selectable_label(
                                soloed,
                                egui::RichText::new(app.i18n.t("track-header-solo"))
                                    .color(solo_color),
                            )
                            .clicked()
                        {
                            soloed = !soloed;
                            app.action_set_track_solo(t_idx, soloed);
                        }
                    });

                    // Pan Slider
                    let mut pan = app.project.tracks[t_idx].pan;
                    if ui
                        .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                        .changed()
                    {
                        app.action_set_track_pan(t_idx, pan);
                    }
                    let pan_str = if pan < -0.05 {
                        app.i18n
                            .t_arg("mixer-pan-left", "value", (-pan * 100.0).round() as i64)
                    } else if pan > 0.05 {
                        app.i18n
                            .t_arg("mixer-pan-right", "value", (pan * 100.0).round() as i64)
                    } else {
                        app.i18n.t("mixer-pan-center")
                    };
                    ui.label(egui::RichText::new(pan_str).size(10.0));

                    // Vertical Gain Fader + Live Peak VU Meter
                    let peak = app.telemetry.track_peaks.get(t_idx).copied().unwrap_or(0.0);
                    let mut gain_db = app.project.tracks[t_idx].gain_db;
                    let mut gain_changed = false;
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Slider::new(&mut gain_db, -60.0..=12.0)
                                    .vertical()
                                    .show_value(false),
                            )
                            .changed()
                        {
                            gain_changed = true;
                        }
                        let (m_rect, _) =
                            ui.allocate_exact_size(Vec2::new(8.0, 90.0), egui::Sense::hover());
                        crate::views::render_vu_meter_bar(ui.painter(), m_rect, peak, 1.0, 2.0);
                    });
                    if gain_changed {
                        app.action_set_track_gain(t_idx, gain_db);
                    }
                    ui.label(
                        egui::RichText::new(format_db(gain_db))
                            .monospace()
                            .size(11.0),
                    );
                    // Routing & Grouping
                    let mut current_routing = app.project.tracks[t_idx].output_routing;
                    let routing_text = match current_routing {
                        stem_core::project::TrackOutputRouting::Master => {
                            app.i18n.t("routing-master")
                        }
                        stem_core::project::TrackOutputRouting::Bus(bid) => app
                            .project
                            .get_bus(bid)
                            .map(|b| format!("-> {}", b.name))
                            .unwrap_or_else(|| app.i18n.t("routing-master")),
                    };
                    egui::ComboBox::from_id_salt(format!(
                        "trk_out_{}",
                        app.project.tracks[t_idx].id
                    ))
                    .selected_text(egui::RichText::new(routing_text).size(9.0))
                    .width(72.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut current_routing,
                            stem_core::project::TrackOutputRouting::Master,
                            app.i18n.t("routing-master"),
                        );
                        for b in &app.project.busses {
                            ui.selectable_value(
                                &mut current_routing,
                                stem_core::project::TrackOutputRouting::Bus(b.id),
                                format!("-> {}", b.name),
                            );
                        }
                    });
                    if current_routing != app.project.tracks[t_idx].output_routing {
                        app.project.tracks[t_idx].output_routing = current_routing;
                        app.sync_timeline_to_audio_engine();
                    }

                    // Group Selector
                    let mut current_gid = app.project.tracks[t_idx].group_id;
                    let grp_text = current_gid
                        .and_then(|gid| app.project.get_group(gid))
                        .map(|g| format!("Grp: {}", g.name))
                        .unwrap_or_else(|| app.i18n.t("group-none"));
                    egui::ComboBox::from_id_salt(format!(
                        "trk_grp_{}",
                        app.project.tracks[t_idx].id
                    ))
                    .selected_text(egui::RichText::new(grp_text).size(9.0))
                    .width(72.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut current_gid, None, app.i18n.t("group-none"));
                        for g in &app.project.groups {
                            ui.selectable_value(
                                &mut current_gid,
                                Some(g.id),
                                format!("Grp: {}", g.name),
                            );
                        }
                        if ui
                            .button(
                                egui::RichText::new(format!("+ {}", app.i18n.t("group-add")))
                                    .size(9.0),
                            )
                            .clicked()
                        {
                            let count = app.project.groups.len() + 1;
                            let new_gid = app.action_add_group(format!("Group {}", count));
                            current_gid = Some(new_gid);
                        }
                    });
                    if current_gid != app.project.tracks[t_idx].group_id {
                        app.project.tracks[t_idx].group_id = current_gid;
                    }

                    // Phase Inversion (ØL / ØR) & Freeze
                    let mut mixer_sync_needed = false;
                    ui.horizontal(|ui| {
                        let trk = &mut app.project.tracks[t_idx];
                        if ui
                            .selectable_label(
                                trk.phase_invert_left,
                                egui::RichText::new("ØL").size(9.0),
                            )
                            .on_hover_text("Invert Left Phase")
                            .clicked()
                        {
                            trk.phase_invert_left = !trk.phase_invert_left;
                            mixer_sync_needed = true;
                        }
                        if ui
                            .selectable_label(
                                trk.phase_invert_right,
                                egui::RichText::new("ØR").size(9.0),
                            )
                            .on_hover_text("Invert Right Phase")
                            .clicked()
                        {
                            trk.phase_invert_right = !trk.phase_invert_right;
                            mixer_sync_needed = true;
                        }
                        if ui
                            .selectable_label(
                                trk.frozen,
                                egui::RichText::new(if trk.frozen { "Frz*" } else { "Frz" })
                                    .size(9.0),
                            )
                            .on_hover_text("Toggle Track Freeze")
                            .clicked()
                        {
                            trk.frozen = !trk.frozen;
                        }
                    });

                    // Stereo Width & Mid/Side
                    let mut width = app.project.tracks[t_idx].stereo_width;
                    if ui
                        .add(
                            egui::Slider::new(&mut width, 0.0..=2.0)
                                .prefix("W:")
                                .fixed_decimals(2)
                                .show_value(true),
                        )
                        .changed()
                    {
                        app.project.tracks[t_idx].stereo_width = width;
                        mixer_sync_needed = true;
                    }
                    let mut mid_db = app.project.tracks[t_idx].mid_gain_db;
                    if ui
                        .add(
                            egui::Slider::new(&mut mid_db, -12.0..=12.0)
                                .prefix("M:")
                                .fixed_decimals(1),
                        )
                        .changed()
                    {
                        app.project.tracks[t_idx].mid_gain_db = mid_db;
                        mixer_sync_needed = true;
                    }
                    let mut side_db = app.project.tracks[t_idx].side_gain_db;
                    if ui
                        .add(
                            egui::Slider::new(&mut side_db, -12.0..=12.0)
                                .prefix("S:")
                                .fixed_decimals(1),
                        )
                        .changed()
                    {
                        app.project.tracks[t_idx].side_gain_db = side_db;
                        mixer_sync_needed = true;
                    }

                    // Sidechain Source Selector
                    let track_id_self = app.project.tracks[t_idx].id;
                    let mut sc_id = app.project.tracks[t_idx].sidechain_source_track_id;
                    let sc_label = sc_id
                        .and_then(|id| app.project.tracks.iter().find(|t| t.id == id))
                        .map(|t| format!("SC: {}", t.name))
                        .unwrap_or_else(|| "SC: Off".to_string());
                    let other_tracks: Vec<(uuid::Uuid, String)> = app
                        .project
                        .tracks
                        .iter()
                        .filter(|t| t.id != track_id_self)
                        .map(|t| (t.id, t.name.clone()))
                        .collect();
                    egui::ComboBox::from_id_salt(format!("trk_sc_{}", track_id_self))
                        .selected_text(egui::RichText::new(sc_label).size(9.0))
                        .width(72.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sc_id, None, "SC: Off");
                            for (oid, oname) in &other_tracks {
                                ui.selectable_value(
                                    &mut sc_id,
                                    Some(*oid),
                                    format!("SC: {}", oname),
                                );
                            }
                        });
                    if sc_id != app.project.tracks[t_idx].sidechain_source_track_id {
                        app.project.tracks[t_idx].sidechain_source_track_id = sc_id;
                        mixer_sync_needed = true;
                    }

                    // Auxiliary Sends
                    if !app.project.busses.is_empty() && app.project.tracks[t_idx].sends.is_empty()
                    {
                        if ui
                            .small_button(egui::RichText::new("+ Send").size(9.0))
                            .clicked()
                        {
                            let first_bus_id = app.project.busses[0].id;
                            app.project.tracks[t_idx]
                                .sends
                                .push(stem_core::project::TrackSend::new(first_bus_id));
                            mixer_sync_needed = true;
                        }
                    } else if let Some(send) = app.project.tracks[t_idx].sends.get_mut(0) {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    send.pre_fader,
                                    egui::RichText::new("Pre").size(8.5),
                                )
                                .on_hover_text("Pre-Fader Send")
                                .clicked()
                            {
                                send.pre_fader = !send.pre_fader;
                                mixer_sync_needed = true;
                            }
                            if ui
                                .add(
                                    egui::Slider::new(&mut send.gain_db, -60.0..=6.0)
                                        .prefix("Snd:")
                                        .fixed_decimals(0),
                                )
                                .changed()
                            {
                                mixer_sync_needed = true;
                            }
                        });
                    }

                    if mixer_sync_needed {
                        app.sync_timeline_to_audio_engine();
                    }
                });
            });
        }

        // ---------------- Submix Busses ----------------
        let mut bus_to_remove: Option<uuid::Uuid> = None;
        for b_idx in 0..app.project.busses.len() {
            let bus = &app.project.busses[b_idx];
            let bus_id = bus.id;
            let bus_name = bus.name.clone();

            ui.group(|ui| {
                ui.set_width(strip_width);
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("BUS")
                                .size(9.0)
                                .color(Color32::from_rgb(100, 180, 240)),
                        );
                        if ui
                            .small_button("×")
                            .on_hover_text(app.i18n.t("bus-remove"))
                            .clicked()
                        {
                            bus_to_remove = Some(bus_id);
                        }
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&bus_name)
                                .strong()
                                .color(Color32::from_rgb(120, 200, 255)),
                        )
                        .truncate(),
                    );

                    // Bus Mute / Solo
                    ui.horizontal(|ui| {
                        let mut muted = app.project.busses[b_idx].muted;
                        let mute_color = if muted {
                            Color32::from_rgb(230, 80, 80)
                        } else {
                            Color32::GRAY
                        };
                        if ui
                            .selectable_label(
                                muted,
                                egui::RichText::new(app.i18n.t("track-header-mute"))
                                    .color(mute_color),
                            )
                            .clicked()
                        {
                            muted = !muted;
                            app.action_set_bus_mute(bus_id, muted);
                        }

                        let mut soloed = app.project.busses[b_idx].soloed;
                        let solo_color = if soloed {
                            Color32::from_rgb(220, 200, 50)
                        } else {
                            Color32::GRAY
                        };
                        if ui
                            .selectable_label(
                                soloed,
                                egui::RichText::new(app.i18n.t("track-header-solo"))
                                    .color(solo_color),
                            )
                            .clicked()
                        {
                            soloed = !soloed;
                            app.action_set_bus_solo(bus_id, soloed);
                        }
                    });

                    // Bus Pan Slider
                    let mut pan = app.project.busses[b_idx].pan;
                    if ui
                        .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                        .changed()
                    {
                        app.action_set_bus_pan(bus_id, pan);
                    }
                    let pan_str = if pan < -0.05 {
                        app.i18n
                            .t_arg("mixer-pan-left", "value", (-pan * 100.0).round() as i64)
                    } else if pan > 0.05 {
                        app.i18n
                            .t_arg("mixer-pan-right", "value", (pan * 100.0).round() as i64)
                    } else {
                        app.i18n.t("mixer-pan-center")
                    };
                    ui.label(egui::RichText::new(pan_str).size(10.0));

                    // Bus Gain Fader
                    let mut gain_db = app.project.busses[b_idx].gain_db;
                    if ui
                        .add(
                            egui::Slider::new(&mut gain_db, -60.0..=12.0)
                                .vertical()
                                .show_value(false),
                        )
                        .changed()
                    {
                        app.action_set_bus_gain(bus_id, gain_db);
                    }
                    ui.label(
                        egui::RichText::new(format_db(gain_db))
                            .monospace()
                            .size(11.0),
                    );

                    // Bus Output Routing
                    let mut bus_out = app.project.busses[b_idx].output_routing;
                    let bus_out_label = match bus_out {
                        stem_core::project::TrackOutputRouting::Master => {
                            app.i18n.t("routing-master")
                        }
                        stem_core::project::TrackOutputRouting::Bus(bid) => app
                            .project
                            .get_bus(bid)
                            .map(|b| format!("-> {}", b.name))
                            .unwrap_or_else(|| app.i18n.t("routing-master")),
                    };
                    let other_busses: Vec<(uuid::Uuid, String)> = app
                        .project
                        .busses
                        .iter()
                        .filter(|b| b.id != bus_id)
                        .map(|b| (b.id, b.name.clone()))
                        .collect();
                    egui::ComboBox::from_id_salt(format!("bus_out_{}", bus_id))
                        .selected_text(egui::RichText::new(bus_out_label).size(9.0))
                        .width(72.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut bus_out,
                                stem_core::project::TrackOutputRouting::Master,
                                app.i18n.t("routing-master"),
                            );
                            for (ob_id, ob_name) in &other_busses {
                                ui.selectable_value(
                                    &mut bus_out,
                                    stem_core::project::TrackOutputRouting::Bus(*ob_id),
                                    format!("-> {}", ob_name),
                                );
                            }
                        });
                    if bus_out != app.project.busses[b_idx].output_routing {
                        app.project.busses[b_idx].output_routing = bus_out;
                        app.sync_timeline_to_audio_engine();
                    }
                });
            });
        }
        if let Some(bid) = bus_to_remove {
            app.action_delete_bus(bid);
        }

        // [+ Bus] Button
        ui.vertical(|ui| {
            if ui
                .button(
                    egui::RichText::new(format!("+ {}", app.i18n.t("bus-add")))
                        .size(11.0)
                        .color(Color32::from_rgb(100, 200, 255)),
                )
                .clicked()
            {
                let count = app.project.busses.len() + 1;
                app.action_add_bus(format!("Bus {}", count));
            }
        });

        ui.separator();

        // ---------------- Master Strip ----------------
        ui.group(|ui| {
            ui.set_width(strip_width + 20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(app.i18n.t("mixer-master-label"))
                        .strong()
                        .color(egui::Color32::from_rgb(255, 180, 50)),
                );

                // Master Fader
                let mut master_db = app.project.master_gain_db;
                if ui
                    .add(
                        egui::Slider::new(&mut master_db, -60.0..=12.0)
                            .vertical()
                            .show_value(false),
                    )
                    .changed()
                {
                    app.action_set_master_gain(master_db);
                }
                ui.label(
                    egui::RichText::new(format_db(master_db))
                        .monospace()
                        .strong()
                        .size(11.0),
                );

                // Peak & RMS Readout
                let peak = app.telemetry.peak_left.max(app.telemetry.peak_right);
                let rms = app.telemetry.rms_left.max(app.telemetry.rms_right);
                let peak_db = if peak > 1e-5 {
                    20.0 * peak.log10()
                } else {
                    -90.0
                };
                let rms_db = if rms > 1e-5 {
                    20.0 * rms.log10()
                } else {
                    -90.0
                };

                let peak_color = if peak_db > 0.0 {
                    Color32::from_rgb(255, 50, 50)
                } else {
                    Color32::from_rgb(100, 220, 100)
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        app.i18n.t("meter-peak"),
                        format_db(peak_db)
                    ))
                    .size(10.0)
                    .color(peak_color),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        app.i18n.t("meter-rms"),
                        format_db(rms_db)
                    ))
                    .size(9.0)
                    .color(Color32::from_rgb(180, 200, 220)),
                );
            });
        });
    });
}

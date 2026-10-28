use crate::error::ProjectError;
use crate::project::{Clip, Project, Track};
use uuid::Uuid;

/// An undoable/redoable action on a Project.
#[derive(Debug, Clone)]
pub enum Action {
    AddTrack(Track),
    RemoveTrack {
        track: Track,
        index: usize,
    },
    AddClip {
        track_id: Uuid,
        clip: Clip,
    },
    RemoveClip {
        track_id: Uuid,
        clip: Clip,
        index: usize,
    },
    MoveClip {
        track_id: Uuid,
        clip_id: Uuid,
        old_track_offset: u64,
        new_track_offset: u64,
    },
    TrimClip {
        track_id: Uuid,
        clip_id: Uuid,
        old_source_start: u64,
        old_length: u64,
        old_track_offset: u64,
        new_source_start: u64,
        new_length: u64,
        new_track_offset: u64,
    },
    SetTrackGain {
        track_id: Uuid,
        old_gain: f32,
        new_gain: f32,
    },
    SetTrackPan {
        track_id: Uuid,
        old_pan: f32,
        new_pan: f32,
    },
    SetMasterGain {
        old_gain: f32,
        new_gain: f32,
    },
}

impl Action {
    pub fn description(&self) -> &'static str {
        match self {
            Action::AddTrack(_) => "Add Track",
            Action::RemoveTrack { .. } => "Remove Track",
            Action::AddClip { .. } => "Add Clip",
            Action::RemoveClip { .. } => "Remove Clip",
            Action::MoveClip { .. } => "Move Clip",
            Action::TrimClip { .. } => "Trim Clip",
            Action::SetTrackGain { .. } => "Change Track Gain",
            Action::SetTrackPan { .. } => "Change Track Pan",
            Action::SetMasterGain { .. } => "Change Master Gain",
        }
    }

    /// Execute the forward action.
    pub fn execute(&self, project: &mut Project) -> Result<(), ProjectError> {
        match self {
            Action::AddTrack(track) => {
                project.tracks.push(track.clone());
            }
            Action::RemoveTrack { index, .. } => {
                if *index < project.tracks.len() {
                    project.tracks.remove(*index);
                }
            }
            Action::AddClip { track_id, clip } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                track.clips.push(clip.clone());
            }
            Action::RemoveClip {
                track_id, index, ..
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if *index < track.clips.len() {
                    track.clips.remove(*index);
                }
            }
            Action::MoveClip {
                track_id,
                clip_id,
                new_track_offset,
                ..
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == *clip_id) {
                    clip.track_offset_frames = *new_track_offset;
                }
            }
            Action::TrimClip {
                track_id,
                clip_id,
                new_source_start,
                new_length,
                new_track_offset,
                ..
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == *clip_id) {
                    clip.source_start_frame = *new_source_start;
                    clip.length_frames = *new_length;
                    clip.track_offset_frames = *new_track_offset;
                }
            }
            Action::SetTrackGain {
                track_id, new_gain, ..
            } => {
                if let Some(track) = project.get_track_mut(*track_id) {
                    track.gain_db = *new_gain;
                }
            }
            Action::SetTrackPan {
                track_id, new_pan, ..
            } => {
                if let Some(track) = project.get_track_mut(*track_id) {
                    track.pan = *new_pan;
                }
            }
            Action::SetMasterGain { new_gain, .. } => {
                project.master_gain_db = *new_gain;
            }
        }
        Ok(())
    }

    /// Roll back the action.
    pub fn undo(&self, project: &mut Project) -> Result<(), ProjectError> {
        match self {
            Action::AddTrack(track) => {
                if let Some(pos) = project.tracks.iter().position(|t| t.id == track.id) {
                    project.tracks.remove(pos);
                }
            }
            Action::RemoveTrack { track, index } => {
                let idx = (*index).min(project.tracks.len());
                project.tracks.insert(idx, track.clone());
            }
            Action::AddClip { track_id, clip } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if let Some(pos) = track.clips.iter().position(|c| c.id == clip.id) {
                    track.clips.remove(pos);
                }
            }
            Action::RemoveClip {
                track_id,
                clip,
                index,
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                let idx = (*index).min(track.clips.len());
                track.clips.insert(idx, clip.clone());
            }
            Action::MoveClip {
                track_id,
                clip_id,
                old_track_offset,
                ..
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == *clip_id) {
                    clip.track_offset_frames = *old_track_offset;
                }
            }
            Action::TrimClip {
                track_id,
                clip_id,
                old_source_start,
                old_length,
                old_track_offset,
                ..
            } => {
                let track = project
                    .get_track_mut(*track_id)
                    .ok_or(ProjectError::TrackNotFound(*track_id))?;
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == *clip_id) {
                    clip.source_start_frame = *old_source_start;
                    clip.length_frames = *old_length;
                    clip.track_offset_frames = *old_track_offset;
                }
            }
            Action::SetTrackGain {
                track_id, old_gain, ..
            } => {
                if let Some(track) = project.get_track_mut(*track_id) {
                    track.gain_db = *old_gain;
                }
            }
            Action::SetTrackPan {
                track_id, old_pan, ..
            } => {
                if let Some(track) = project.get_track_mut(*track_id) {
                    track.pan = *old_pan;
                }
            }
            Action::SetMasterGain { old_gain, .. } => {
                project.master_gain_db = *old_gain;
            }
        }
        Ok(())
    }
}

/// Managing undo/redo history.
#[derive(Debug, Default)]
pub struct UndoRedoStack {
    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,
    max_history: usize,
}

impl UndoRedoStack {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 100,
        }
    }

    pub fn push_and_execute(
        &mut self,
        action: Action,
        project: &mut Project,
    ) -> Result<(), ProjectError> {
        action.execute(project)?;
        self.undo_stack.push(action);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self, project: &mut Project) -> Result<Option<Action>, ProjectError> {
        if let Some(action) = self.undo_stack.pop() {
            action.undo(project)?;
            self.redo_stack.push(action.clone());
            Ok(Some(action))
        } else {
            Ok(None)
        }
    }

    pub fn redo(&mut self, project: &mut Project) -> Result<Option<Action>, ProjectError> {
        if let Some(action) = self.redo_stack.pop() {
            action.execute(project)?;
            self.undo_stack.push(action.clone());
            Ok(Some(action))
        } else {
            Ok(None)
        }
    }

    pub fn history(&self) -> &[Action] {
        &self.undo_stack
    }
}

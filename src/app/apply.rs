use crate::protocol::{build_bulk_write_messages, build_write_messages, ProtocolError};

use super::AppState;

pub fn build_apply_messages(state: &AppState) -> Option<Result<Vec<Vec<u8>>, ProtocolError>> {
    let snapshot = state.snapshot.as_ref()?;
    let mut frames = Vec::new();

    if !state.staged_edits.is_empty() {
        match build_write_messages(snapshot, &state.staged_edits) {
            Ok(messages) => frames.extend(messages),
            Err(err) => return Some(Err(err)),
        }
    }

    if let (Some(device_colors), Some(staged_colors)) =
        (state.pad_colors.as_ref(), state.staged_pad_colors.as_ref())
    {
        let mut writes = Vec::new();
        if staged_colors.inactive != device_colors.inactive {
            if let Some(buffer) = staged_colors.inactive.as_deref() {
                writes.push((1u8, buffer));
            }
        }
        if staged_colors.active != device_colors.active {
            if let Some(buffer) = staged_colors.active.as_deref() {
                writes.push((2u8, buffer));
            }
        }

        if !writes.is_empty() {
            match build_bulk_write_messages(&snapshot.family, &writes) {
                Ok(messages) => frames.extend(messages),
                Err(err) => return Some(Err(err)),
            }
        }
    }

    Some(Ok(frames))
}

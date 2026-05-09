use crate::error::ApiError;

pub const STATE_DRAFT: &str = "draft";
pub const STATE_OPEN: &str = "open";
pub const STATE_PROCESSING: &str = "processing";
pub const STATE_PAID: &str = "paid";
pub const STATE_VOID: &str = "void";
pub const STATE_UNCOLLECTIBLE: &str = "uncollectible";

pub fn is_valid_state(state: &str) -> bool {
    matches!(
        state,
        STATE_DRAFT | STATE_OPEN | STATE_PROCESSING | STATE_PAID | STATE_VOID | STATE_UNCOLLECTIBLE
    )
}

pub fn is_terminal_state(state: &str) -> bool {
    matches!(state, STATE_PAID | STATE_VOID | STATE_UNCOLLECTIBLE)
}

pub fn can_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        (STATE_DRAFT, STATE_OPEN)
            | (STATE_DRAFT, STATE_VOID)
            | (STATE_OPEN, STATE_PROCESSING)
            | (STATE_PROCESSING, STATE_PAID)
            | (STATE_PROCESSING, STATE_OPEN)
            | (STATE_OPEN, STATE_VOID)
            | (STATE_OPEN, STATE_UNCOLLECTIBLE)
    )
}

pub fn validate_transition(from: &str, to: &str) -> Result<(), ApiError> {
    if !is_valid_state(from) || !is_valid_state(to) {
        return Err(ApiError::bad_request(
            "invalid_state",
            "unknown invoice state",
        ));
    }

    if from == to {
        return Ok(());
    }

    if !can_transition(from, to) {
        return Err(ApiError::conflict(
            "invalid_state_transition",
            format!("invalid transition from '{from}' to '{to}'"),
        ));
    }

    Ok(())
}

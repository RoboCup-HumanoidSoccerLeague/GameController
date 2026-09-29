//! This module implements the error reporting of the C API: Functions that fail return NULL or
//! false and store a message that can be obtained with [gc_last_error].

use std::{
    cell::RefCell,
    ffi::{c_char, CString},
    fmt::Display,
};

use game_controller_core::types::PlayerNumber;

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// This function stores the message of an error that occurred on the current thread.
pub(crate) fn set_last_error(message: impl Display) {
    // Interior null bytes would truncate the message, so they are removed.
    let message = CString::new(message.to_string().replace('\0', "")).unwrap_or_default();
    LAST_ERROR.with(|last_error| *last_error.borrow_mut() = message);
}

/// This function converts a player number from C, or stores an error if it is out of range.
pub(crate) fn player_number(player: u8) -> Option<PlayerNumber> {
    if (PlayerNumber::MIN..=PlayerNumber::MAX).contains(&player) {
        Some(PlayerNumber::new(player))
    } else {
        set_last_error(format!(
            "invalid player number {player} (must be {}-{})",
            PlayerNumber::MIN,
            PlayerNumber::MAX
        ));
        None
    }
}

/// This function returns a message describing the last error on the calling thread.
///
/// The message is empty if no error has occurred yet. The string is owned by the library and
/// remains valid until the next error occurs on the same thread.
#[no_mangle]
pub extern "C" fn gc_last_error() -> *const c_char {
    LAST_ERROR.with(|last_error| last_error.borrow().as_ptr())
}

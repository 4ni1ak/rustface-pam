use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use super::constants::{PAM_SUCCESS, PAM_USER};

// PAM handle — opaque C struct, used as a pointer only
pub enum PamHandle {}

// pam_get_item is provided by the PAM linker
extern "C" {
    fn pam_get_item(
        pamh: *const PamHandle,
        item_type: c_int,
        item: *mut *const c_void,
    ) -> c_int;
}

/// Get the username from the PAM handle.
///
/// # Safety
/// `pamh` must be a valid PAM handle pointer.
pub unsafe fn get_username(pamh: *const PamHandle) -> Option<String> {
    let mut ptr: *const c_void = std::ptr::null();

    let ret = pam_get_item(pamh, PAM_USER, &mut ptr);

    if ret != PAM_SUCCESS || ptr.is_null() {
        return None;
    }

    // SAFETY: PAM returns a null-terminated C string for PAM_USER
    CStr::from_ptr(ptr as *const c_char)
        .to_str()
        .ok()
        .map(|s| s.to_owned())
}

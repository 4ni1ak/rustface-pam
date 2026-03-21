use std::os::raw::c_int;

pub const PAM_SUCCESS: c_int = 0;
pub const PAM_AUTH_ERR: c_int = 7;
pub const PAM_IGNORE: c_int = 25;
pub const PAM_USER: c_int = 2;

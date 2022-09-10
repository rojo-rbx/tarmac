//! Implementation of automatically fetching authentication cookie from a Roblox
//! Studio installation.

use secrecy::SecretString;

#[cfg(windows)]
pub fn get_auth_cookie() -> Option<SecretString> {
    rbx_cookie::get().map(|f| SecretString::new(f))
}

#[cfg(not(windows))]
pub fn get_auth_cookie() -> Option<String> {
    rbx_cookie::get().map(|f| SecretString::new(f))
}

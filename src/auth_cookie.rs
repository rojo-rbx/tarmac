//! Implementation of automatically fetching authentication cookie from a Roblox
//! Studio installation.

use secrecy::SecretString;

pub fn get_auth_cookie() -> Option<SecretString> {
    rbx_cookie::get().map(|f| SecretString::new(f))
}

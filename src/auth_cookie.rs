//! Implementation of automatically fetching authentication cookie from a Roblox
//! Studio installation.

use reqwest::{
    header::{self, HeaderValue},
    Client,
};
use secrecy::{ExposeSecret, SecretString};

use crate::roblox_api::RobloxApiError;

pub fn get_auth_cookie() -> Option<SecretString> {
    rbx_cookie::get_value().map(SecretString::new)
}

pub fn get_csrf_token(roblosecurity_cookie: &SecretString) -> Result<HeaderValue, RobloxApiError> {
    let response = Client::new()
        .post("https://auth.roblox.com")
        .header(header::COOKIE, roblosecurity_cookie.expose_secret())
        .header(header::CONTENT_LENGTH, 0)
        .send();

    response
        .unwrap()
        .headers()
        .get("X-CSRF-Token")
        .map(|v| v.to_owned())
        .ok_or(RobloxApiError::MissingCsrfToken)
}

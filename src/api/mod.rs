use reqwest::StatusCode;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, env};
use thiserror::Error;

use crate::{auth_cookie::get_auth_cookie, options::GlobalOptions};

use self::{opencloud::OpenCloudClient, roblox_web::RobloxApiClient};

pub mod opencloud;
pub mod roblox_web;

#[derive(Debug, Clone)]
pub struct ImageUploadData<'a> {
    pub image_data: Cow<'a, [u8]>,
    pub name: &'a str,
    pub description: &'a str,
    pub group_id: Option<u64>,
    pub user_id: Option<u64>,
}

/// Internal representation of what the asset upload endpoint returns, before
/// we've handled any errors.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawUploadResponse {
    pub success: bool,
    pub message: Option<String>,
    pub asset_id: Option<u64>,
    pub backing_asset_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UploadResponse {
    pub asset_id: u64,
    pub backing_asset_id: u64,
}

#[derive(Debug, Error)]
pub enum RobloxApiError {
    #[error("Roblox API HTTP error")]
    Http {
        #[from]
        source: reqwest::Error,
    },

    #[error("Roblox OpenCloud error")]
    RbxCloud {
        #[from]
        source: rbxcloud::rbx::error::Error,
    },

    #[error("Roblox API error: {message}")]
    ApiError { message: String },

    #[error("Roblox API returned success, but had malformed JSON response: {body}")]
    BadResponseJson {
        body: String,
        source: serde_json::Error,
    },

    #[error("Roblox API returned HTTP {status} with body: {body}")]
    ResponseError { status: StatusCode, body: String },

    #[error("Request for CSRF token did not return an X-CSRF-Token header.")]
    MissingCsrfToken,
}

pub trait Api {
    fn download_image(&mut self, id: u64) -> Result<Vec<u8>, RobloxApiError>;

    /// Upload an image, retrying if the asset endpoint determines that the
    /// asset's name is inappropriate. The asset's name will be replaced with a
    /// generic known-good string.
    fn upload_image_with_moderation_retry(
        &mut self,
        data: ImageUploadData,
    ) -> Result<UploadResponse, RobloxApiError>;

    /// Upload an image, returning an error if anything goes wrong.
    fn upload_image(&mut self, data: ImageUploadData) -> Result<UploadResponse, RobloxApiError>;
}

pub enum Clients {
    OpenCloud(OpenCloudClient),
    RobloxApi(RobloxApiClient),
}

pub fn get_client(options: GlobalOptions) -> Clients {
    if let Some(api_key) = options
        .api_key
        .or(env::var("TARMAC_API_KEY").ok().map(SecretString::new))
    {
        Clients::OpenCloud(OpenCloudClient::new(api_key))
    } else {
        Clients::RobloxApi(RobloxApiClient::new(
            options.cookie.or_else(get_auth_cookie),
        ))
    }
}

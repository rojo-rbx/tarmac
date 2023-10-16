use std::fmt::{self, Write};

use reqwest::{
    header::{HeaderValue, COOKIE},
    Client, Request, Response, StatusCode,
};
use secrecy::{ExposeSecret, SecretString};

use crate::auth_cookie::get_csrf_token;

use super::{Api, ImageUploadData, RawUploadResponse, RobloxApiError, UploadResponse};

pub struct RobloxApiClient {
    auth_token: Option<SecretString>,
    csrf_token: Option<HeaderValue>,
    client: Client,
}

impl fmt::Debug for RobloxApiClient {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "RobloxApiClient")
    }
}

impl Api for RobloxApiClient {
    fn download_image(&mut self, id: u64) -> Result<Vec<u8>, RobloxApiError> {
        let url = format!("https://roblox.com/asset?id={}", id);

        let mut response =
            self.execute_with_csrf_retry(|client| Ok(client.get(&url).build()?))?;

        let mut buffer = Vec::new();
        response.copy_to(&mut buffer)?;

        Ok(buffer)
    }

    fn upload_image_with_moderation_retry(
        &mut self,
        data: ImageUploadData,
    ) -> Result<UploadResponse, RobloxApiError> {
        let response = self.upload_image_raw(&data)?;

        // Some other errors will be reported inside the response, even
        // though we received a successful HTTP response.
        if response.success {
            let asset_id = response.asset_id.unwrap();
            let backing_asset_id = response.backing_asset_id.unwrap();

            Ok(UploadResponse {
                asset_id,
                backing_asset_id,
            })
        } else {
            let message = response.message.unwrap();

            // There are no status codes for this API, so we pattern match
            // on the returned error message.
            //
            // If the error message text mentions something being
            // inappropriate, we assume the title was problematic and
            // attempt to re-upload.
            if message.contains("inappropriate") {
                log::warn!(
                    "Image name '{}' was moderated, retrying with different name...",
                    data.name
                );

                let new_data = ImageUploadData {
                    name: "image",
                    ..data
                };

                self.upload_image(new_data)
            } else {
                Err(RobloxApiError::ApiError { message })
            }
        }
    }

    fn upload_image(&mut self, data: ImageUploadData) -> Result<UploadResponse, RobloxApiError> {
        let response = self.upload_image_raw(&data)?;

        // Some other errors will be reported inside the response, even
        // though we received a successful HTTP response.
        if response.success {
            let asset_id = response.asset_id.unwrap();
            let backing_asset_id = response.backing_asset_id.unwrap();

            Ok(UploadResponse {
                asset_id,
                backing_asset_id,
            })
        } else {
            let message = response.message.unwrap();

            Err(RobloxApiError::ApiError { message })
        }
    }
}

impl RobloxApiClient {
    pub fn new(auth_token: Option<SecretString>) -> Self {
        match auth_token {
            Some(token) => {
                let csrf_token = match get_csrf_token(&token) {
                    Ok(value) => Some(value),
                    Err(err) => {
                        log::error!("Was unable to fetch CSRF token: {}", err.to_string());
                        None
                    }
                };

                Self {
                    auth_token: Some(token),
                    csrf_token,
                    client: Client::new(),
                }
            }
            _ => Self {
                auth_token,
                csrf_token: None,
                client: Client::new(),
            },
        }
    }

    fn upload_image_raw(
        &mut self,
        data: &ImageUploadData,
    ) -> Result<RawUploadResponse, RobloxApiError> {
        let mut url = "https://data.roblox.com/data/upload/json?assetTypeId=13".to_owned();

        if let Some(group_id) = data.group_id {
            write!(url, "&groupId={}", group_id).unwrap();
        }

        let mut response = self.execute_with_csrf_retry(|client| {
            Ok(client
                .post(&url)
                .query(&[("name", data.name), ("description", data.description)])
                .body(data.image_data.clone().into_owned())
                .build()?)
        })?;

        let body = response.text()?;

        // Some errors will be reported through HTTP status codes, handled here.
        if response.status().is_success() {
            match serde_json::from_str(&body) {
                Ok(response) => Ok(response),
                Err(source) => Err(RobloxApiError::BadResponseJson { body, source }),
            }
        } else {
            Err(RobloxApiError::ResponseError {
                status: response.status(),
                body,
            })
        }
    }

    /// Execute a request generated by the given function, retrying if the
    /// endpoint requests that the user refreshes their CSRF token.
    fn execute_with_csrf_retry<F>(&mut self, make_request: F) -> Result<Response, RobloxApiError>
    where
        F: Fn(&Client) -> Result<Request, RobloxApiError>,
    {
        let mut request = make_request(&self.client)?;
        self.attach_headers(&mut request);

        let response = self.client.execute(request)?;

        match response.status() {
            StatusCode::FORBIDDEN => {
                if let Some(csrf) = response.headers().get("X-CSRF-Token") {
                    log::debug!("Retrying request with X-CSRF-Token...");

                    self.csrf_token = Some(csrf.clone());

                    let mut new_request = make_request(&self.client)?;
                    self.attach_headers(&mut new_request);

                    Ok(self.client.execute(new_request)?)
                } else {
                    // If the response did not return a CSRF token for us to
                    // retry with, this request was likely forbidden for other
                    // reasons.

                    Ok(response)
                }
            }
            _ => Ok(response),
        }
    }

    /// Attach required headers to a request object before sending it to a
    /// Roblox API, like authentication and CSRF protection.
    fn attach_headers(&self, request: &mut Request) {
        if let Some(auth_token) = &self.auth_token {
            let cookie_value = format!(".ROBLOSECURITY={}", auth_token.expose_secret());

            request.headers_mut().insert(
                COOKIE,
                HeaderValue::from_bytes(cookie_value.as_bytes()).unwrap(),
            );
        }

        if let Some(csrf) = &self.csrf_token {
            request.headers_mut().insert("X-CSRF-Token", csrf.clone());
        }
    }
}

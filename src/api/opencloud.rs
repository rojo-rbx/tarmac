use std::env;

use rbxcloud::rbx::assets::{
    AssetCreation, AssetCreationContext, AssetCreator, AssetGroupCreator, AssetOperation,
    AssetType, AssetUserCreator,
};
use reqwest::{multipart, Client, Response};
use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Deserialize};

use super::{roblox_web::RobloxApiClient, Api, RobloxApiError};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetGetOperation {
    pub path: String,
    pub done: Option<bool>,
    pub response: Option<AssetGetOperationResponse>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetGetOperationResponse {
    #[serde(rename = "@type")]
    pub response_type: Option<String>,
    pub path: String,
    pub revision_id: String,
    pub revision_create_time: String,
    pub asset_id: String,
    pub display_name: String,
    pub description: String,
    pub asset_type: String,
    pub creation_context: AssetCreationContext,
}

pub struct OpenCloudClient {
    api_key: SecretString,
    client: Client,
}

fn handle_res<T: DeserializeOwned>(mut res: Response) -> Result<T, RobloxApiError> {
    let status = res.status();
    match status.is_success() {
        true => {
            let body = res.json::<T>()?;
            Ok(body)
        }
        false => {
            let text = res.text().unwrap();
            Err(RobloxApiError::ResponseError { status, body: text })
        }
    }
}

impl Api for OpenCloudClient {
    fn download_image(&mut self, _id: u64) -> Result<Vec<u8>, RobloxApiError> {
        // Fallback onto the web api for downloading
        let mut roblox_api_client = RobloxApiClient::new(None);
        roblox_api_client.download_image(15090277769)
    }

    fn upload_image(
        &mut self,
        data: super::ImageUploadData,
    ) -> Result<super::UploadResponse, RobloxApiError> {
        let asset = AssetCreation {
            asset_type: AssetType::DecalPng,
            display_name: data.name.into(),
            description: data.description.into(),
            creation_context: AssetCreationContext {
                creator: data
                    .group_id
                    .map(|group_id| {
                        AssetCreator::Group(AssetGroupCreator {
                            group_id: group_id.to_string(),
                        })
                    })
                    .unwrap_or(AssetCreator::User(AssetUserCreator {
                        user_id: data
                            .user_id
                            .map(|id| id.to_string())
                            .or(env::var("TARMAC_USER_ID").ok())
                            .expect("No user_id - Either need a user_id or group_id!"),
                    })),
                expected_price: None,
            },
        };

        let asset_json = serde_json::to_string(&asset).unwrap();
        let asset_file = multipart::Part::bytes(data.image_data.clone().into_owned())
            .file_name(data.name.clone().to_owned())
            .mime_str("image/png")?;

        let form = multipart::Form::new()
            .text("request", asset_json)
            .part("fileContent", asset_file);

        let response = self
            .client
            .post("https://apis.roblox.com/assets/v1/assets")
            .header("x-api-key", self.api_key.expose_secret())
            .multipart(form)
            .send()?;

        let result = handle_res::<AssetOperation>(response)?;
        let url = format!(
            "https://apis.roblox.com/assets/v1/{operationId}",
            operationId = result.path.expect("No operationId path!")
        );

        // Continue making a GET for the asset until we get a response.
        loop {
            let response = self
                .client
                .get(&url)
                .header("x-api-key", self.api_key.expose_secret())
                .send()?;

            let result = handle_res::<AssetGetOperation>(response)?;

            if let Some(response) = result.response {
                let asset_id: u64 = response.asset_id.parse().expect(&format!(
                    "Failed to parse asset_id ({}) as a number!",
                    response.asset_id
                ));

                return Ok(super::UploadResponse {
                    asset_id,
                    backing_asset_id: asset_id,
                });
            }
        }
    }

    fn upload_image_with_moderation_retry(
        &mut self,
        data: super::ImageUploadData,
    ) -> Result<super::UploadResponse, RobloxApiError> {
        self.upload_image(data)
    }
}

impl OpenCloudClient {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }
}

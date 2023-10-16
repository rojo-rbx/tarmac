use rbxcloud::rbx::assets::{
    AssetCreation, AssetCreationContext, AssetCreator, AssetGetOperation, AssetGroupCreator,
    AssetOperation, AssetType, AssetUserCreator,
};
use reqwest::{multipart, Client, Response};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;

use super::{Api, RobloxApiError};

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
        todo!("Downloading images not implmented for Open Cloud!");
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
                        user_id: "1091164489".into(),
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

        std::thread::sleep(std::time::Duration::from_secs(3));

        let url = format!(
            "https://apis.roblox.com/assets/v1/{operationId}",
            operationId = result.path.unwrap()
        );
        let response = self
            .client
            .get(&url)
            .header("x-api-key", self.api_key.expose_secret())
            .send()?;

        let result = handle_res::<AssetGetOperation>(response)?;
        let response = result.response;

        let asset_id: u64 = response.asset_id.parse().unwrap();

        Ok(super::UploadResponse {
            asset_id,
            backing_asset_id: asset_id,
        })
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

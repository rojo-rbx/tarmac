use rbxcloud::rbx::assets::{
    AssetCreation, AssetCreationContext, AssetCreator, AssetOperation, AssetType,
};
use reqwest::multipart;
use secrecy::SecretString;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::sync_backend::Error;

pub struct TarmacCloudAsset {
    asset: AssetCreation,
    contents: Vec<u8>,
}

impl TarmacCloudAsset {
    pub fn from_bytes(
        creator: AssetCreator,
        asset_type: AssetType,
        name: String,
        contents: Vec<u8>,
    ) -> Self {
        Self {
            contents,
            asset: AssetCreation {
                asset_type,
                display_name: name,
                description: "Uploaded by tarmac".into(),
                creation_context: AssetCreationContext {
                    creator,
                    expected_price: None,
                },
            },
        }
    }
}

fn build_url(asset_id: Option<u64>) -> String {
    if let Some(asset_id) = asset_id {
        format!("https://apis.roblox.com/assets/v1/assets/{asset_id}")
    } else {
        "https://apis.roblox.com/assets/v1/assets".to_string()
    }
}

fn handle_res<T: DeserializeOwned>(mut res: reqwest::Response) -> Result<T, RobloxCloudError> {
    let status = res.status();
    match status.is_success() {
        true => {
            let body = res.json::<T>()?;
            Ok(body)
        }
        false => {
            let text = res.text()?;
            Err(RobloxCloudError::HttpStatusError {
                code: status.as_u16(),
                message: text,
            })
        }
    }
}

#[derive(Debug, Error)]
pub enum RobloxCloudError {
    #[error(transparent)]
    SerdeJsonError {
        #[from]
        source: serde_json::Error,
    },

    #[error(transparent)]
    ReqwestError {
        #[from]
        source: reqwest::Error,
    },

    #[error("HttpError")]
    HttpStatusError { code: u16, message: String },

    #[error(transparent)]
    RobloxCloudError {
        #[from]
        source: rbxcloud::rbx::error::Error,
    },
}

/// Upload using RbxCloud
pub struct RbxCloudApi {
    api_key: String,
}

impl RbxCloudApi {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub fn upload(&self, asset: TarmacCloudAsset) -> Result<AssetOperation, RobloxCloudError> {
        let asset_info = serde_json::to_string(&asset.asset)?;
        let file: multipart::Part = multipart::Part::bytes(asset.contents);
        let form = multipart::Form::new()
            .text("request", asset_info)
            .part("fileContent", file);

        // Create new asset - https://create.roblox.com/docs/cloud/open-cloud/usage-assets#creating-an-new-asset
        let client = reqwest::Client::new();
        let url = build_url(None);
        let res = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .multipart(form)
            .send()?;

        let upload_response = handle_res::<AssetOperation>(res)?;
        if let Some(path) = upload_response.path {
            // Check uploaded asset - https://create.roblox.com/docs/cloud/open-cloud/usage-assets#checking-an-uploaded-asset
            client
                .get(&format!(
                    "https://apis.roblox.com/assets/v1/{operation_id}",
                    operation_id = path
                ))
                .header("x-api-key", &self.api_key);

            panic!("TODO");
        } else {
            panic!("idk");
        }
    }
}

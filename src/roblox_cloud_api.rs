use std::{collections::HashMap, fs, io, path::PathBuf};

use rbxcloud::rbx::assets::{
    AssetCreation, AssetCreationContext, AssetCreator, AssetErrorStatus, AssetOperation, AssetType,
    ProtobufAny,
};
use reqwest::multipart;
use secrecy::SecretString;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{data::AssetId, sync_backend::Error};

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
                    expected_price: Some(0),
                },
            },
        }
    }

    pub fn from_file(
        creator: AssetCreator,
        asset_type: AssetType,
        name: String,
        file_path: PathBuf,
    ) -> Result<Self, io::Error> {
        let contents = fs::read(&file_path)?;
        Ok(Self::from_bytes(creator, asset_type, name, contents))
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

    /// Attempts to upload the given asset using the cloud API
    pub fn upload(
        &self,
        name: String,
        cloud_asset: TarmacCloudAsset,
    ) -> Result<Option<AssetId>, RobloxCloudError> {
        let asset_info = serde_json::to_string(&cloud_asset.asset)?;
        let file: multipart::Part = multipart::Part::bytes(cloud_asset.contents)
            .file_name(format!("{}.png", name))
            .mime_str("image/png")?;

        let form = multipart::Form::new()
            .text("request", asset_info)
            .part("fileContent", file);

        println!("{:#?}", form);

        // Create new asset - https://create.roblox.com/docs/cloud/open-cloud/usage-assets#creating-an-new-asset
        let client = reqwest::Client::new();
        let url = build_url(None);
        let upload_res = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .multipart(form)
            .send()?;

        // Retrieve the operation result - see above URL link for more info
        let upload_operation = handle_res::<AssetOperation>(upload_res)?;
        if let Some(path) = upload_operation.path {
            println!("op_path = {:#?}", path);

            // Check uploaded asset - https://create.roblox.com/docs/cloud/open-cloud/usage-assets#checking-an-uploaded-asset
            let check_res = client
                .get(&format!(
                    "https://apis.roblox.com/assets/v1/{operation_id}",
                    operation_id = path
                ))
                .header("x-api-key", &self.api_key)
                .send()?;

            let check_operation = handle_res::<AssetCreateResponseOperation>(check_res)?;

            println!("{:#?}", check_operation);
            // if let Some(response) = check_operation.response {
            //     let id_str = response.get_asset_id();
            //     return Ok(id_str);
            // }

            panic!("TODO");
        } else {
            panic!("idk");
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetCreateResponseOperation {
    pub path: Option<String>,
    pub metadata: Option<ProtobufAny>,
    pub done: Option<bool>,
    pub error: Option<AssetErrorStatus>,
    pub response: Option<AssetOperationResponse>,
}

#[derive(Deserialize, Debug)]
pub struct AssetOperationResponse {
    #[serde(rename = "@type")]
    pub message_type: String,
    // pub path: Option<String>,
    // pub asset_id: Option<String>,

    #[serde(flatten)]
    pub rest: HashMap<String, Value>,
}

impl AssetOperationResponse {
    // pub fn get_asset_id(self) -> Option<AssetId> {
    //     self.asset_id.map(|f| AssetId::Id(f.parse().unwrap()))
    // }
}

mod tests {
    use std::env;

    #[test]
    fn test_upload() {
        use super::{RbxCloudApi, TarmacCloudAsset};
        use rbxcloud::rbx::assets::*;
        use std::path::PathBuf;

        let asset = TarmacCloudAsset::from_file(
            AssetCreator::User(AssetUserCreator {
                user_id: "4308133".into(),
            }),
            AssetType::DecalPng,
            "Test Tarmac v0.8.0 Cloud API".into(),
            PathBuf::from("examples/01-basic-game/assets/logo.png"),
        )
        .unwrap();

        let upload = RbxCloudApi::new(env::var("TEST_TARMAC_API_KEY").unwrap());
        let result = upload
            .upload("logo".into(), asset)
            .expect("Could not upload");
        // println!("{:#?}", serde_json::to_string(&asset.asset));

        println!("{:#?}", result);
    }
}

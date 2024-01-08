use std::time::Duration;

use rbxcloud::rbx::{
    assets::{
        AssetCreation, AssetCreationContext, AssetCreator, AssetGroupCreator, AssetType,
        AssetUserCreator,
    },
    error::Error as RbxCloudError,
    CreateAssetWithContents, GetAsset, RbxAssets, RbxCloud,
};
use reqwest::StatusCode;
use secrecy::ExposeSecret;
use tokio::runtime::Runtime;

use super::{
    legacy::LegacyClient, ImageUploadData, RobloxApiClient, RobloxApiError, RobloxCredentials,
    UploadResponse,
};

pub struct OpenCloudClient {
    credentials: RobloxCredentials,
    creator: AssetCreator,
    assets: RbxAssets,
    runtime: Runtime,
}

impl RobloxApiClient for OpenCloudClient {
    fn new(credentials: RobloxCredentials) -> Result<Self, RobloxApiError> {
        let creator = match (credentials.group_id, credentials.user_id) {
            (Some(id), None) => Ok(AssetCreator::Group(AssetGroupCreator {
                group_id: id.to_string(),
            })),
            (None, Some(id)) => Ok(AssetCreator::User(AssetUserCreator {
                user_id: id.to_string(),
            })),
            (None, None) => Err(RobloxApiError::ApiKeyNeedsCreatorId),
            (Some(_), Some(_)) => Err(RobloxApiError::AmbiguousCreatorType),
        }?;

        let assets = RbxCloud::new(
            credentials
                .api_key
                .as_ref()
                .ok_or(RobloxApiError::MissingAuth)?
                .expose_secret(),
        )
        .assets();

        Ok(Self {
            creator,
            assets,
            credentials,
            runtime: Runtime::new().unwrap(),
        })
    }

    fn upload_image_with_moderation_retry(
        &mut self,
        data: &ImageUploadData,
    ) -> Result<UploadResponse, RobloxApiError> {
        match self.upload_image(data) {
            Err(RobloxApiError::ResponseError { status, body })
                if status == 400 && body.contains("moderated") =>
            {
                log::warn!(
                    "Image name '{}' was moderated, retrying with different name...",
                    data.name
                );
                self.upload_image(&ImageUploadData {
                    name: "image",
                    ..data.to_owned()
                })
            }

            result => result,
        }
    }

    fn upload_image(&mut self, data: &ImageUploadData) -> Result<UploadResponse, RobloxApiError> {
        self.upload_image_inner(data)
    }

    fn download_image(&mut self, id: u64) -> Result<Vec<u8>, RobloxApiError> {
        LegacyClient::new(self.credentials.clone())?.download_image(id)
    }
}

impl OpenCloudClient {
    fn upload_image_inner(&self, data: &ImageUploadData) -> Result<UploadResponse, RobloxApiError> {
        let asset_info = CreateAssetWithContents {
            asset: AssetCreation {
                asset_type: AssetType::DecalPng,
                display_name: data.name.to_string(),
                description: data.description.to_string(),
                creation_context: AssetCreationContext {
                    creator: self.creator.clone(),
                    expected_price: None,
                },
            },
            contents: &data.image_data,
        };

        let operation_id = self
            .runtime
            .block_on(async { self.assets.create_with_contents(&asset_info).await })
            .map(|response| response.path)?
            .ok_or(RobloxApiError::MissingOperationPath)?
            .strip_prefix("operations/")
            .ok_or(RobloxApiError::MalformedOperationPath)?
            .to_string();

        const MAX_RETRIES: u32 = 5;
        const INITIAL_SLEEP_DURATION: Duration = Duration::from_millis(50);
        const BACKOFF: u32 = 2;

        let mut retry_count = 0;
        let operation = GetAsset { operation_id };
        let asset_id = loop {
            let maybe_asset_id = self
                .runtime
                .block_on(async { self.assets.get(&operation).await })?
                .response
                .map(|response| response.asset_id)
                .map(|id| id.parse::<u64>().map_err(RobloxApiError::MalformedAssetId));

            match maybe_asset_id {
                Some(id) => break id,
                None if retry_count > MAX_RETRIES => break Err(RobloxApiError::AssetGetFailed),

                _ => {
                    retry_count += 1;
                    std::thread::sleep(INITIAL_SLEEP_DURATION * retry_count.pow(BACKOFF));
                }
            }
        }?;

        Ok(UploadResponse {
            asset_id,
            backing_asset_id: asset_id,
        })
    }
}

impl From<RbxCloudError> for RobloxApiError {
    fn from(value: RbxCloudError) -> Self {
        match value {
            RbxCloudError::HttpStatusError { code, msg } => RobloxApiError::ResponseError {
                status: StatusCode::from_u16(code).unwrap_or_default(),
                body: msg,
            },
            _ => RobloxApiError::RbxCloud(value),
        }
    }
}

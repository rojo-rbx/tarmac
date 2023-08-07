use core::panic;
use std::path::{Path, PathBuf};
use std::{env, fs};


use rbxcloud::rbx::assets::{AssetCreator, AssetGroupCreator, AssetType};

use crate::roblox_cloud_api::{TarmacCloudAsset, RbxCloudApi};

use super::SyncBackend;

pub struct RobloxCloudBackend {
    api: RbxCloudApi,
    creator: AssetCreator,
}

impl RobloxCloudBackend {
    pub fn new(api: RbxCloudApi, creator: AssetCreator) -> Self {
        Self {
            api,
            creator,
        }
    }
}

impl SyncBackend for RobloxCloudBackend {
    fn upload(&mut self, data: super::UploadInfo) -> Result<super::UploadResponse, super::Error> {        
        let asset = TarmacCloudAsset::from_bytes(self.creator.clone(), AssetType::DecalPng, data.name, data.contents);
        let result = self.api.upload(asset).unwrap();

        panic!("TODO");
    }
}

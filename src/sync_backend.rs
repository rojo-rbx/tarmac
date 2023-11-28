use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use fs_err as fs;
use reqwest::StatusCode;
use roblox_install::RobloxStudio;
use thiserror::Error;

use crate::api::{ImageUploadData, RobloxApiError};
use crate::{api::Api, data::AssetId};

pub trait SyncBackend {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResponse {
    pub id: AssetId,
}

#[derive(Clone, Debug)]
pub struct UploadInfo {
    pub name: String,
    pub contents: Vec<u8>,
    pub hash: String,
}

pub struct RobloxSyncBackend<'a, Client: Api> {
    api_client: &'a mut Client,
    upload_to_group_id: Option<u64>,
    upload_to_user_id: Option<u64>,
}

impl<'a, Client: Api> RobloxSyncBackend<'a, Client> {
    pub fn new(
        api_client: &'a mut Client,
        upload_to_group_id: Option<u64>,
        upload_to_user_id: Option<u64>,
    ) -> Self {
        Self {
            api_client,
            upload_to_group_id,
            upload_to_user_id,
        }
    }
}

impl<'a, Client: Api> SyncBackend for RobloxSyncBackend<'a, Client> {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        log::info!("Uploading {} to Roblox", &data.name);

        let result = self
            .api_client
            .upload_image_with_moderation_retry(ImageUploadData {
                image_data: Cow::Owned(data.contents),
                name: &data.name,
                description: "Uploaded by Tarmac.",
                group_id: self.upload_to_group_id,
                user_id: self.upload_to_user_id,
            });

        match result {
            Ok(response) => {
                log::info!(
                    "Uploaded {} to ID {}",
                    &data.name,
                    response.backing_asset_id
                );

                Ok(UploadResponse {
                    id: AssetId::Id(response.backing_asset_id),
                })
            }

            Err(RobloxApiError::ResponseError {
                status: StatusCode::TOO_MANY_REQUESTS,
                ..
            }) => Err(Error::RateLimited),

            Err(err) => Err(err.into()),
        }
    }
}

pub struct LocalSyncBackend {
    content_path: PathBuf,
    scope: Option<String>,
}

impl LocalSyncBackend {
    pub fn new(scope: Option<String>) -> Result<LocalSyncBackend, Error> {
        RobloxStudio::locate()
            .map(|studio| LocalSyncBackend {
                content_path: studio.content_path().into(),
                scope,
            })
            .map_err(|error| error.into())
    }

    fn get_asset_path(&self, data: &UploadInfo) -> PathBuf {
        let mut path = PathBuf::from(".tarmac");
        if let Some(scope) = &self.scope {
            path.push(scope);
        }
        path.push(self.get_asset_file_name(data));
        path
    }

    fn get_asset_file_name(&self, data: &UploadInfo) -> String {
        format!("{}.png", data.name)
    }
}

impl SyncBackend for LocalSyncBackend {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        let asset_path = self.get_asset_path(&data);
        let file_path = self.content_path.join(&asset_path);
        let parent = file_path
            .parent()
            .expect("content folder should have a parent");

        fs::create_dir_all(parent)?;
        fs::write(&file_path, &data.contents)?;

        log::info!("Written {} to path {}", &data.name, file_path.display());

        Ok(UploadResponse {
            id: AssetId::Path(asset_path),
        })
    }
}

pub struct NoneSyncBackend;

impl SyncBackend for NoneSyncBackend {
    fn upload(&mut self, _data: UploadInfo) -> Result<UploadResponse, Error> {
        Err(Error::NoneBackend)
    }
}

pub struct DebugSyncBackend {
    last_id: u64,
}

impl DebugSyncBackend {
    pub fn new() -> Self {
        Self { last_id: 0 }
    }
}

impl SyncBackend for DebugSyncBackend {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        log::info!("Copying {} to local folder", &data.name);

        self.last_id += 1;
        let id = self.last_id;

        let path = Path::new(".tarmac-debug");
        fs::create_dir_all(path)?;

        let file_path = path.join(id.to_string());
        fs::write(file_path, &data.contents)?;

        Ok(UploadResponse {
            id: AssetId::Id(id),
        })
    }
}

/// Performs the retry logic for rate limitation errors. The struct wraps a SyncBackend so that
/// when a RateLimited error occurs, the thread sleeps for a moment and then tries to reupload the
/// data.
pub struct RetryBackend<InnerSyncBackend> {
    inner: InnerSyncBackend,
    delay: Duration,
    attempts: usize,
}

impl<InnerSyncBackend> RetryBackend<InnerSyncBackend> {
    /// Creates a new backend from another SyncBackend. The max_retries parameter gives the number
    /// of times the backend will try again (so given 0, it acts just as the original SyncBackend).
    /// The delay parameter provides the amount of time to wait between each upload attempt.
    pub fn new(inner: InnerSyncBackend, max_retries: usize, delay: Duration) -> Self {
        Self {
            inner,
            delay,
            attempts: max_retries + 1,
        }
    }
}

impl<InnerSyncBackend: SyncBackend> SyncBackend for RetryBackend<InnerSyncBackend> {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        for index in 0..self.attempts {
            if index != 0 {
                log::info!(
                    "tarmac is being rate limited, retrying upload ({}/{})",
                    index,
                    self.attempts - 1
                );
                thread::sleep(self.delay);
            }
            let result = self.inner.upload(data.clone());

            match result {
                Err(Error::RateLimited) => {}
                _ => return result,
            }
        }

        Err(Error::RateLimited)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Cannot upload assets with the 'none' target.")]
    NoneBackend,

    #[error("Tarmac was rate-limited trying to upload assets. Try again in a little bit.")]
    RateLimited,

    #[error(transparent)]
    StudioInstall {
        #[from]
        source: roblox_install::Error,
    },

    #[error(transparent)]
    Io {
        #[from]
        source: io::Error,
    },

    #[error(transparent)]
    RobloxError {
        #[from]
        source: RobloxApiError,
    },
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(unused_must_use)]
    mod test_retry_backend {
        use super::*;

        struct CountUploads<'a> {
            counter: &'a mut usize,
            results: Vec<Result<UploadResponse, Error>>,
        }

        impl<'a> CountUploads<'a> {
            fn new(counter: &'a mut usize) -> Self {
                Self {
                    counter,
                    results: Vec::new(),
                }
            }

            fn with_results(mut self, results: Vec<Result<UploadResponse, Error>>) -> Self {
                self.results = results;
                self.results.reverse();
                self
            }
        }

        impl<'a> SyncBackend for CountUploads<'a> {
            fn upload(&mut self, _data: UploadInfo) -> Result<UploadResponse, Error> {
                (*self.counter) += 1;
                self.results.pop().unwrap_or(Err(Error::NoneBackend))
            }
        }

        fn any_upload_info() -> UploadInfo {
            UploadInfo {
                name: "foo".to_owned(),
                contents: Vec::new(),
                hash: "hash".to_owned(),
            }
        }

        fn retry_duration() -> Duration {
            Duration::from_millis(1)
        }

        #[test]
        fn upload_at_least_once() {
            let mut counter = 0;
            let mut backend =
                RetryBackend::new(CountUploads::new(&mut counter), 0, retry_duration());

            backend.upload(any_upload_info());

            assert_eq!(counter, 1);
        }

        #[test]
        fn upload_again_if_rate_limited() {
            let mut counter = 0;
            let inner = CountUploads::new(&mut counter).with_results(vec![
                Err(Error::RateLimited),
                Err(Error::RateLimited),
                Err(Error::NoneBackend),
            ]);
            let mut backend = RetryBackend::new(inner, 5, retry_duration());

            backend.upload(any_upload_info());

            assert_eq!(counter, 3);
        }

        #[test]
        fn upload_returns_first_success_result() {
            let mut counter = 0;
            let success = UploadResponse {
                id: AssetId::Id(10),
            };
            let inner = CountUploads::new(&mut counter).with_results(vec![
                Err(Error::RateLimited),
                Err(Error::RateLimited),
                Ok(success.clone()),
            ]);
            let mut backend = RetryBackend::new(inner, 5, retry_duration());

            let upload_result = backend.upload(any_upload_info()).unwrap();

            assert_eq!(counter, 3);
            assert_eq!(upload_result, success);
        }

        #[test]
        fn upload_returns_rate_limited_when_retries_exhausted() {
            let mut counter = 0;
            let inner = CountUploads::new(&mut counter).with_results(vec![
                Err(Error::RateLimited),
                Err(Error::RateLimited),
                Err(Error::RateLimited),
                Err(Error::RateLimited),
            ]);
            let mut backend = RetryBackend::new(inner, 2, retry_duration());

            let upload_result = backend.upload(any_upload_info()).unwrap_err();

            assert_eq!(counter, 3);
            assert!(matches!(upload_result, Error::RateLimited));
        }
    }
}

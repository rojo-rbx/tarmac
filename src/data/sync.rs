use std::{fmt, path::PathBuf};

use crate::{
    asset_name::AssetName,
    data::{ImageSlice, InputConfig, InputManifest},
};

use path_slash::PathBufExt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetId {
    Id(u64),
    Path(PathBuf),
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}",
            match &self {
                Self::Id(id) => format!("rbxassetid://{}", id),
                Self::Path(path) => format!(
                    "rbxasset://{}",
                    path.to_slash()
                        .expect("error while converting path to slash")
                ),
            }
        )
    }
}

/// In-memory representation of a Tarmac Input during the sync process.
///
/// SyncInput structs are gradually created and filled in from the filesystem,
/// results of network I/O, and from the previous Tarmac manifest file.
#[derive(Debug)]
pub struct SyncInput {
    /// A unique name for this asset in the project.
    pub name: AssetName,

    /// The path on disk to the file this input originated from.
    pub path: PathBuf,

    /// The input's path with DPI scale information stripped away. This is used
    /// to group inputs that are just DPI variations of eachother.
    pub path_without_dpi_scale: PathBuf,

    /// The DPI scale of this input, if it makes sense for this input type.
    pub dpi_scale: u32,

    /// The configuration that applied to this input when it was discovered.
    pub config: InputConfig,

    /// The contents of the file this input originated from.
    pub contents: Vec<u8>,

    /// A hash of `contents`.
    pub hash: String,

    /// If this input has been part of an upload to Roblox.com, contains the
    /// asset ID that contains the data from this input.
    pub id: Option<AssetId>,

    /// If this input has been packed into a spritesheet, contains the slice of
    /// the spritesheet that this input is located in.
    pub slice: Option<ImageSlice>,
}

impl SyncInput {
    pub fn is_unchanged_since_last_sync(&self, old_manifest: &InputManifest) -> bool {
        self.hash == old_manifest.hash && self.config.packable == old_manifest.packable
    }

    /// Creates a non-unique, human-friendly name to refer to this input.
    pub fn human_name(&self) -> String {
        let file_stem = self
            .path_without_dpi_scale
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();

        if self.path == self.path_without_dpi_scale {
            file_stem.to_owned()
        } else {
            format!("{} ({}x)", file_stem, self.dpi_scale)
        }
    }
}

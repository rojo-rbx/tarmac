use std::{
    collections::BTreeMap,
    io,
    path::{self, Path},
};

use crate::{data::SyncInput, lua, typescript};

pub fn perform_codegen(output_path: Option<&Path>, inputs: &[&SyncInput]) -> io::Result<()> {
    lua::codegen::perform_codegen(output_path, inputs)?;
    typescript::codegen::perform_codegen(output_path, inputs)?;

    Ok(())
}

/// Tree used to track and group inputs hierarchically, before turning them into
/// Lua tables.
#[derive(Debug)]
pub enum GroupedItem<'a> {
    Folder {
        children_by_name: BTreeMap<String, GroupedItem<'a>>,
    },
    InputGroup {
        inputs_by_dpi_scale: BTreeMap<u32, &'a SyncInput>,
    },
}

impl GroupedItem<'_> {
    pub fn parse_root_folder<'a>(
        output_path: &Path,
        inputs: &'a [&SyncInput],
    ) -> BTreeMap<String, GroupedItem<'a>> {
        let mut root_folder: BTreeMap<String, GroupedItem<'a>> = BTreeMap::new();

        for &input in inputs {
            // Not all inputs will be marked for codegen. We can eliminate those
            // right away.
            if !input.config.codegen {
                continue;
            }

            // The extension portion of the path is not useful for code generation.
            // By stripping it off, we generate the names that users expect.
            let mut path_without_extension = input.path_without_dpi_scale.clone();
            path_without_extension.set_extension("");

            // If we can't construct a relative path, there isn't a sensible name
            // that we can use to refer to this input.
            let relative_path = path_without_extension
                .strip_prefix(&input.config.codegen_base_path)
                .expect("Input base path was not a base path for input");

            // Collapse `..` path segments so that we can map this path onto our
            // tree of inputs.
            let mut segments = Vec::new();
            for component in relative_path.components() {
                match component {
                    path::Component::Prefix(_)
                    | path::Component::RootDir
                    | path::Component::Normal(_) => {
                        segments.push(component.as_os_str().to_str().unwrap())
                    }
                    path::Component::CurDir => {}
                    path::Component::ParentDir => assert!(segments.pop().is_some()),
                }
            }

            // Navigate down the tree, creating any folder entries that don't exist
            // yet.
            let mut current_dir = &mut root_folder;
            for (i, &segment) in segments.iter().enumerate() {
                if i == segments.len() - 1 {
                    // We assume that the last segment of a path must be a file.

                    let input_group = match current_dir.get_mut(segment) {
                        Some(existing) => existing,
                        None => {
                            let input_group = GroupedItem::InputGroup {
                                inputs_by_dpi_scale: BTreeMap::new(),
                            };
                            current_dir.insert(segment.to_owned(), input_group);
                            current_dir.get_mut(segment).unwrap()
                        }
                    };

                    if let GroupedItem::InputGroup {
                        inputs_by_dpi_scale,
                    } = input_group
                    {
                        inputs_by_dpi_scale.insert(input.dpi_scale, input);
                    } else {
                        unreachable!();
                    }
                } else {
                    let next_entry = current_dir.entry(segment.to_owned()).or_insert_with(|| {
                        GroupedItem::Folder {
                            children_by_name: BTreeMap::new(),
                        }
                    });

                    if let GroupedItem::Folder { children_by_name } = next_entry {
                        current_dir = children_by_name;
                    } else {
                        unreachable!();
                    }
                }
            }
        }

        root_folder
    }
}

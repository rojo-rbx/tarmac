use fs_err as fs;

use image::{codecs::png::PngEncoder, GenericImageView};

use std::borrow::Cow;

use crate::{
    alpha_bleed::alpha_bleed,
    auth_cookie::get_auth_cookie,
    options::{GlobalOptions, UploadImageOptions},
    roblox_web_api::{ImageUploadData, RobloxApiClient},
};

pub fn upload_image(global: GlobalOptions, options: UploadImageOptions) {
    let auth = global
        .auth
        .or_else(get_auth_cookie)
        .expect("no auth cookie found");

    let image_data = fs::read(options.path).expect("couldn't read input file");

    let mut img = image::load_from_memory(&image_data).expect("couldn't load image");

    alpha_bleed(&mut img);

    let (width, height) = img.dimensions();

    let mut encoded_image: Vec<u8> = Vec::new();
    PngEncoder::new(&mut encoded_image)
        .encode(&img.to_bytes(), width, height, img.color())
        .unwrap();

    let mut client = RobloxApiClient::new(Some(auth));

    let upload_data = ImageUploadData {
        image_data: Cow::Owned(encoded_image.to_vec()),
        name: &options.name,
        description: &options.description,
        group_id: None,
    };

    let response = client
        .upload_image(upload_data)
        .expect("Roblox API request failed");

    eprintln!("Image uploaded successfully!");
    println!("{}", response.backing_asset_id);
}

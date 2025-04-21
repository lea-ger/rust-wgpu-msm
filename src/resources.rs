/*
    Code taken from https://sotrh.github.io/learn-wgpu/beginner/tutorial9-models/#accessing-files-from-wasm
    Making this project ready for rendering on the web is not in the scope of this project, I'd just like to keep this as an option for the future.
 */
use std::env;
use cfg_if::cfg_if;

use crate::texture;

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let mut origin = location.origin().unwrap();
    if !origin.ends_with("learn-wgpu") {
        origin = format!("{}/learn-wgpu", origin);
    }
    let base = reqwest::Url::parse(&format!("{}/", origin,)).unwrap();
    base.join(file_name).unwrap()
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let dir = env::current_dir()?;
            let path = std::path::Path::new(&dir)
                .join(file_name);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let dir = env::current_dir()?;
            let path = std::path::Path::new(&dir)
                .join(file_name);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}


pub async fn load_texture(
    file_name: Option<&str>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<texture::Texture> {
    if file_name.is_none() {
        return Err(anyhow::anyhow!("No file name provided"));
    }
    let file_name = file_name.as_ref().unwrap();
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}


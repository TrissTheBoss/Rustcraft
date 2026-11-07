//! Local-only extraction of user-owned client visual resources.
use anyhow::{Context, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

const BLOCK_PREFIX: &str = "assets/minecraft/textures/block/";
const ITEM_PREFIX: &str = "assets/minecraft/textures/item/";
const CACHE_VERSION: &str = "block-textures-v1";

/// Ensures the local visual cache contains the textures currently consumed by
/// the prototype. This is strictly offline: it only reads a locally installed
/// client JAR and writes the ignored project cache.
pub fn ensure_cached(output: &Path) -> Result<bool> {
    let marker = output.join(".cache-version");
    let required = [
        output.join("textures/blocks/dirt.png"),
        output.join("textures/blocks/grass_block_side.png"),
        output.join("textures/blocks/grass_block_top.png"),
        output.join("textures/blocks/stone.png"),
    ];
    if marker.is_file() && required.iter().all(|path| path.is_file()) {
        return Ok(false);
    }
    let count = extract(None, output)?;
    fs::write(&marker, CACHE_VERSION)
        .with_context(|| format!("cannot write texture cache marker {}", marker.display()))?;
    Ok(count > 0)
}

pub fn extract(jar: Option<&Path>, output: &Path) -> Result<usize> {
    let jar = jar
        .map(PathBuf::from)
        .or_else(find_default_jar)
        .context("could not locate a local client JAR; pass its path explicitly")?;
    let file = fs::File::open(&jar)
        .with_context(|| format!("cannot open client JAR {}", jar.display()))?;
    let mut archive = ZipArchive::new(file).context("client file is not a readable ZIP/JAR")?;
    let blocks = output.join("textures/blocks");
    let items = output.join("textures/items");
    let mut extracted = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).context("cannot read JAR entry")?;
        let name = entry.name().replace('\\', "/");
        let destination = if let Some(relative) = name.strip_prefix(BLOCK_PREFIX) {
            if relative.is_empty() {
                continue;
            }
            blocks.join(relative)
        } else if let Some(relative) = name.strip_prefix(ITEM_PREFIX) {
            if relative.is_empty() {
                continue;
            }
            items.join(relative)
        } else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let mut output_file = fs::File::create(&destination)
            .with_context(|| format!("cannot write {}", destination.display()))?;
        std::io::copy(&mut entry, &mut output_file)
            .with_context(|| format!("cannot extract {}", name))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn find_default_jar() -> Option<PathBuf> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)?;
    let candidates = if cfg!(windows) {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        Some(home.join("Library/Application Support/minecraft"))
    } else {
        [
            home.join(".minecraft"),
            home.join(".var/app/com.mojang.Minecraft/.minecraft"),
        ]
        .into_iter()
        .find(|path| path.is_dir())
    }?;
    let versions = candidates.join("versions");
    let mut jars = Vec::new();
    for version in fs::read_dir(versions).ok()? {
        let path = version.ok()?.path();
        if path.is_dir() {
            let jar = path.join(format!("{}.jar", path.file_name()?.to_string_lossy()));
            if jar.is_file() {
                jars.push(jar);
            }
        }
    }
    jars.sort();
    jars.pop()
}

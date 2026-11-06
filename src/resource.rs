//! Original resource format boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceManifest {
    pub assets: Vec<AssetId>,
}

impl ResourceManifest {
    pub fn empty() -> Self {
        Self { assets: Vec::new() }
    }
}

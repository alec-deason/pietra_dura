use amethyst::{
    core::Transform,
    renderer::{sprite::prefab::SpriteSheetReference, sprite::Sprites},
};
use rendy::hal::image::SamplerInfo;
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct PrefabEntity<P> {
    pub data: Option<P>
}
#[derive(Serialize, Debug)]
pub struct Prefab<P> {
    pub entities: Vec<PrefabEntity<P>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageFormat {
    pub sampler_info: SamplerInfo
}

#[derive(Debug, Clone, Serialize)]
pub enum TexturePrefab {
    File(String, (String, ImageFormat)),
}
#[derive(Debug, Clone, Serialize)]
pub enum SpriteSheetPrefab {
    Sheet {
        texture: TexturePrefab,
        sprites: Vec<Sprites>,
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct SpriteRenderPrefab {
    pub sheet: Option<SpriteSheetReference>,
    pub sprite_number: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpriteScenePrefab {
    pub sheet: Option<SpriteSheetPrefab>,
    pub render: Option<SpriteRenderPrefab>,
    pub transform: Option<Transform>,
}

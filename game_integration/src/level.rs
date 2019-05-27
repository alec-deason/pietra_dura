use amethyst::{
    assets::{PrefabData, ProgressCounter},
    core::Transform,
    derive::PrefabData,
    ecs::prelude::*,
    error::Error,
    renderer::sprite::prefab::{SpriteRenderPrefab, SpriteSheetPrefab},
};
use serde::{Deserialize, Serialize};
use specs_derive::Component;

pub type LevelPrefabHandle = amethyst::assets::Handle<amethyst::assets::Prefab<LevelPrefab>>;

#[derive(Debug, Clone, Deserialize, Serialize, PrefabData)]
pub struct LevelPrefab {
    sheet: Option<SpriteSheetPrefab>,
    render: Option<SpriteRenderPrefab>,
    transform: Option<Transform>,
    #[prefab(Component)]
    detail: Tile,
}

#[derive(Default, Debug, Copy, Clone, Component, Serialize, Deserialize)]
pub struct Tile;

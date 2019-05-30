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

use pietra_dura_nphysics::{
    PhysicsEntityPrefab
};

#[cfg(feature = "asset-prep")]
use pietra_dura_tiled::{
    TiledConverter, SpriteContext, SpriteSheetPrefab as SpriteSheetPrefabProxy, SpriteRenderPrefab as SpriteRenderPrefabProxy
};
#[cfg(feature = "asset-prep")]
use pietra_dura_nphysics::{
    CollisionGroupPrefab, ColliderPrefab, ShapePrefab
};
#[cfg(feature = "asset-prep")]
use amethyst::{
    renderer::sprite::prefab::SpriteSheetReference,
};
#[cfg(feature = "asset-prep")]
use tiled::{Object, ObjectShape};

pub type LevelPrefabHandle = amethyst::assets::Handle<amethyst::assets::Prefab<LevelPrefab>>;

#[derive(Debug, Clone, Deserialize, Serialize, PrefabData)]
pub struct LevelPrefab {
    sheet: Option<SpriteSheetPrefab>,
    render: Option<SpriteRenderPrefab>,
    transform: Option<Transform>,
    detail: Detail,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum CollisionTypes {
    Main = 0,
    Other = 1,
}
impl Into<usize> for CollisionTypes {
    fn into(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PrefabData)]
pub enum Detail {
    Tile(#[prefab(Component)] Tile),
    StaticSprite(#[prefab(Component)] StaticSprite),
    Physics(PhysicsEntityPrefab<CollisionTypes>),
}
#[derive(Default, Debug, Copy, Clone, Component, Serialize, Deserialize)]
pub struct Tile;

#[derive(Default, Debug, Copy, Clone, Component, Serialize, Deserialize)]
pub struct StaticSprite;

// Because a number of important Prefabs in Amethyst are currently impossible
// to construct or serialize outside of amethyst itself we will actually
// produce a proxy type which will result in the same RON output as the real
// types would.
//
// Some types, like `Transform`, serialize perfectly so we can use those directly.
#[cfg(feature = "asset-prep")]
#[derive(Debug, Clone, Serialize)]
pub struct LevelPrefabProxy {
    pub sheet: Option<SpriteSheetPrefabProxy>,
    pub render: Option<SpriteRenderPrefabProxy>,
    pub transform: Option<Transform>,
    pub detail: Detail,

}
#[cfg(feature = "asset-prep")]
impl TiledConverter<'_, LevelPrefab> for LevelPrefab {
    type PrefabProxy = LevelPrefabProxy;

    fn convert_tile(ctx: &Option<SpriteContext>, x: f32, y: f32, layer: usize) -> Option<Self::PrefabProxy> {
        // The SpriteContext contains information about the sprite sheet which this tile
        // references. If ctx is None then this is an empty tile.
        if let Some(ctx) = ctx {
            let render = SpriteRenderPrefabProxy {
                sheet: Some(SpriteSheetReference::Name(ctx.name.clone())),
                sprite_number: ctx.sprite_id as usize,
            };
            let mut transform = Transform::default();
            transform.set_translation_xyz(
                x,
                // In tiled's coordinate system down is positive, so flip it.
                -y,
                layer as f32,
            );
            Some(Self::PrefabProxy {
                // ctx.sprite_sheet will be Some(SpriteSheetPrefabProxy) if
                // this is the first time this sheet is being referenced
                // otherwise it will be None. You shouldn't have to worry
                // about it unless you're doing something complicated.
                sheet: ctx.sprite_sheet.clone(),
                render: Some(render),
                transform: Some(transform),
                detail: Detail::Tile(Tile),
            })
        } else {
            None
        }
    }

    fn convert_object(ctx: &Option<SpriteContext>, layer: usize, object: &Object) -> Option<Self::PrefabProxy> {
        // If ctx is None then this object doesn't have a tile image associated with it.
        match object.obj_type.as_ref() {
            "static" => {
                if let Some(ctx) = ctx {
                    if let ObjectShape::Rect { width, height } = object.shape {
                        let render = SpriteRenderPrefabProxy {
                            sheet: Some(SpriteSheetReference::Name(ctx.name.clone())),
                            sprite_number: ctx.sprite_id as usize,
                        };
                        let mut transform = Transform::default();
                        transform.set_translation_xyz(
                            object.x + width / 2.0,
                            -object.y + height / 2.0,
                            layer as f32,
                        );
                        return Some(Self::PrefabProxy {
                            sheet: ctx.sprite_sheet.clone(),
                            render: Some(render),
                            transform: Some(transform),
                            detail: Detail::StaticSprite(StaticSprite),
                        })
                    }
                }
            },
            "dynamic" => {
                if let Some(ctx) = ctx {
                    if let ObjectShape::Rect { width, height } = object.shape {
                        let render = SpriteRenderPrefabProxy {
                            sheet: Some(SpriteSheetReference::Name(ctx.name.clone())),
                            sprite_number: ctx.sprite_id as usize,
                        };
                        let mut transform = Transform::default();
                        transform.set_translation_xyz(
                            object.x + width / 2.0,
                            -object.y + height / 2.0,
                            layer as f32,
                        );
                        return Some(Self::PrefabProxy {
                            sheet: ctx.sprite_sheet.clone(),
                            render: Some(render),
                            transform: Some(transform),
                            detail: Detail::Physics(PhysicsEntityPrefab {
                                colliders: vec![ColliderPrefab {
                                    // Assume that everything is a circle
                                    shape: ShapePrefab::Ball { radius: width / 2.0 },
                                    density: 1.0,
                                    restitution: 0.8,
                                    friction: 0.5,
                                    offset_x: 0.0,
                                    offset_y: 0.0,
                                    is_sensor: false,
                                    collision_group: CollisionGroupPrefab {
                                        membership: vec![CollisionTypes::Main],
                                        whitelist: vec![CollisionTypes::Main],
                                        blacklist: vec![],
                                    },
                                    location: None,
                                }],
                                collider_only: false,
                                gravity_enabled: true,
                                no_rotate: false,
                                location: Some((object.x + width / 2.0, -object.y + height / 2.0)),
                            }),
                        })
                    }
                }
            },
            "collision" => {
                if let ObjectShape::Rect { width, height } = object.shape {
                    return Some(Self::PrefabProxy {
                        sheet: None,
                        render: None,
                        transform: None,
                        detail: Detail::Physics(PhysicsEntityPrefab {
                            colliders: vec![ColliderPrefab {
                                shape: ShapePrefab::Rect { width, height },
                                density: 1.0,
                                restitution: 0.8,
                                friction: 0.5,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                is_sensor: false,
                                collision_group: CollisionGroupPrefab {
                                    membership: vec![CollisionTypes::Main],
                                    whitelist: vec![CollisionTypes::Main],
                                    blacklist: vec![],
                                },
                                location: Some((object.x + width / 2.0, -object.y - height / 2.0)),
                            }],
                            collider_only: true,
                            gravity_enabled: false,
                            no_rotate: false,
                            location: None,
                        }),
                    })
                }
            },
            _ => (),
        }
        None
    }
}

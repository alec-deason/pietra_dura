mod prefabs;

use std::collections::{HashSet, HashMap};
use image::{
    png::PNGEncoder,
    ColorType,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::fs::File;
use std::io::Result;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::fs::{copy, create_dir_all};

use amethyst::{
    renderer::{
        formats::texture::ImageFormat,
        sprite::{
            prefab::{SpriteSheetPrefab as RealSpriteSheetPrefab, SpriteRenderPrefab as RealSpriteRenderPrefab, SpriteSheetReference},
            SpriteList, SpritePosition, Sprites
        },
    },
    ecs::prelude::*,
    derive::PrefabData,
    core::Transform,
    assets::{PrefabData, ProgressCounter},
    error::Error,
};
use specs_derive::Component;
use sheep::{AmethystFormat, InputSprite, SimplePacker};


pub use prefabs::*;

use tiled::{Tile, Object, Map, parse_file};

pub struct SpriteContext {
    pub sprite_sheet: Option<SpriteSheetPrefab>,
    pub name: String,
    pub sprite_id: usize,
}

pub trait MapObject {
}

pub trait FromTile {
    fn convert(tile: &Tile, x: f32, y: f32, layer: usize, context: &SpriteContext) -> Self;
}

pub trait FromObject {
    fn convert(tile: &Object, layer: usize, context: Option<&SpriteContext>) -> Self;
}

pub enum MapFile {
    Path(PathBuf, PathBuf),
    Data(PathBuf, Vec<u8>),
}
pub trait TiledConverter<'s, P> 
    where P: PrefabData<'s> {
        type PrefabProxy: Serialize;

        fn from_map(input: &Path, map_prefix: &Path) -> MapPrefab<P, Self::PrefabProxy>;
        fn convert_tile(ctx: &SpriteContext, x: f32, y: f32, layer: usize) -> Option<Self::PrefabProxy>;
        fn convert_object(object_type: &str, ctx: Option<&SpriteContext>, layer: usize) -> Option<Self::PrefabProxy>;
}

pub struct MapPrefab<Prefab, Proxy> {
    phantom_prefab: PhantomData<Prefab>,
    phantom_proxy: PhantomData<Proxy>,
    files: Vec<MapFile>,
}

impl<Prefab, Proxy> MapPrefab<Prefab, Proxy> {
    pub fn new(files: Vec<MapFile>) -> Self {
        MapPrefab {
            phantom_prefab: PhantomData,
            phantom_proxy: PhantomData,
            files: files,
        }
    }

    pub fn write(&self, dir: &Path) -> Result<()> {
        for file in self.files() {
            match file {
                MapFile::Path(src, dest) => {
                    let dest = dir.join(dest);
                    create_dir_all(dest.parent().expect("No path?")).expect("Unable to create output directories");
                    copy(src, dest).expect("Unable to copy spritesheet image");
                },
                MapFile::Data(dest, buffer) => {
                    let dest = dir.join(dest);
                    create_dir_all(dest.parent().expect("No path?")).expect("Unable to create output directories");
                    let mut f = File::create(dest).expect("Unable to create target sprite sheet image file");
                    f.write(&buffer).expect("Unable to write sprite sheet image");
                },
            }
        }
        Ok(())
    }

    pub fn files(&self) -> &[MapFile] {
        &self.files
    }
}

pub struct SimpleConverter<P> {
    phantom: PhantomData<P>,
    files: Vec<MapFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PrefabData)]
pub struct RealLevelPrefab {
    sheet: Option<RealSpriteSheetPrefab>,
    render: Option<RealSpriteRenderPrefab>,
    transform: Option<Transform>,
    #[prefab(Component)]
    detail: MapTile,
}

#[derive(Default, Debug, Copy, Clone, Component, Serialize, Deserialize)]
pub struct MapTile;


impl TiledConverter<'_, RealLevelPrefab> for SimpleConverter<RealLevelPrefab> {
    type PrefabProxy = LevelPrefab;

    fn convert_tile(ctx: &SpriteContext, x: f32, y: f32, layer: usize) -> Option<Self::PrefabProxy> {
        let render = SpriteRenderPrefab {
            sheet: Some(SpriteSheetReference::Name(ctx.name.clone())),
            sprite_number: ctx.sprite_id,
        };
        let mut transform = Transform::default();
        transform.set_translation_xyz(
            x,
            y,
            layer as f32,
        );
        Some(LevelPrefab {
            sheet: ctx.sprite_sheet.clone(),
            render: Some(render),
            transform: Some(transform),
            detail: Tile,
        })
    }

    fn convert_object(object_type: &str, ctx: Option<&SpriteContext>, layer: usize) -> Option<Self::PrefabProxy> {
        None
    }

    fn from_map(input: &Path, map_prefix: &Path) -> MapPrefab<RealLevelPrefab, Self::PrefabProxy> {
        let input_dir = input.parent().unwrap();
        let map = parse_file(input).unwrap();

        let (mut sprite_files, sprite_sheets): (Vec<MapFile>, Vec<SpriteSheetPrefab>) = sprite_sheets_from_tilesets(&map, &input_dir, map_prefix).drain(..).unzip();

        let mut used_spritesheets = HashSet::new();
        let mut entities = Vec::new();
        for (z, layer) in map.layers.iter().enumerate() {
            for (y, row) in layer.tiles.iter().enumerate() {
                for (x, gid) in row.iter().enumerate() {
                    if *gid == 0 {
                        continue;
                    }
                    let mut sprite_sheet_id = 0;
                    let mut sprite_id = 0;
                    for (i, tileset) in map.tilesets.iter().enumerate() {
                        if tileset.first_gid > *gid {
                            break;
                        }
                        sprite_sheet_id = i; 
                        sprite_id = *gid as usize - tileset.first_gid as usize;
                    }
                    let sprite_sheet = if used_spritesheets.contains(&sprite_sheet_id) {
                        None
                    } else {
                        used_spritesheets.insert(sprite_sheet_id);
                        Some(sprite_sheets[sprite_sheet_id].clone())
                    };
                    let ctx = SpriteContext {
                        sprite_sheet,
                        name: format!("map_sprite_sheet_{}", sprite_sheet_id),
                        sprite_id,
                    };
                    if let Some(tile) = Self::convert_tile(&ctx, x as f32 * map.tile_width as f32 + map.tile_width as f32 / 2.0, -(y as f32) * map.tile_height as f32 - map.tile_height as f32 / 2.0, z) {
                        entities.push(
                            PrefabEntity { data: Some(tile) }
                        );
                    }
                }
            }
        }

        let mut files:Vec<MapFile> = sprite_files.drain(..).enumerate().filter(|(i, _)| used_spritesheets.contains(i)).map(|(_, f)| f).collect();

        let map = Prefab { entities: entities }; 
        let buffer = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
            .expect("Failed to encode map prefab file");
        files.push(MapFile::Data(PathBuf::from("map.ron"), buffer.into_bytes()));

        MapPrefab::new(files)
    }
}

pub fn sprite_sheets_from_tilesets(map: &Map, input_dir: &Path, map_prefix: &Path) -> Vec<(MapFile, SpriteSheetPrefab)> {
    let mut spritesheets = Vec::new();
    for (i, tileset) in map.tilesets.iter().enumerate() {
        let base_path = PathBuf::from(format!("sprite_sheet_{}", i));
        if !tileset.images.is_empty() {
            let img = &tileset.images[0];
            let texture_path = base_path.with_extension("png");

            let mut sprites = vec![];
            let tileset_sprite_columns = img.width / tileset.tile_width as i32;
            let tileset_sprite_rows = img.height / tileset.tile_height as i32;

            for x in (0..tileset_sprite_rows).rev() {
                for y in 0..tileset_sprite_columns {
                    sprites.push(SpritePosition {
                        y: x as u32 * tileset.tile_width as u32,
                        x: y as u32 * tileset.tile_height as u32,
                        width: tileset.tile_width as u32,
                        height: tileset.tile_height as u32,
                        offsets: None,
                        flip_horizontal: false,
                        flip_vertical: false,
                    });
                }
            }
            let sprite_sheet = SpriteSheetPrefab::Sheet {
                texture: TexturePrefab::File(
                    map_prefix.join(&texture_path).to_str().unwrap().to_string(),
                    (
                        "IMAGE".to_string(),
                        Box::new(ImageFormat::default()).0.sampler_info,
                    ),
                ),
                sprites: vec![Sprites::List(SpriteList {
                    texture_width: img.width as u32,
                    texture_height: img.height as u32,
                    sprites: sprites,
                })],
                name: Some(format!("map_sprite_sheet_{}", i)),
            };
            spritesheets.push((MapFile::Path(input_dir.join(&img.source), texture_path), sprite_sheet));
        } else {
            let mut images = Vec::with_capacity(tileset.tiles.len());
            for tile in &tileset.tiles {
                //TODO tiles can have multiple images because of animation probably. Ignoring that.
                let img = image::open(input_dir.join(&tile.images[0].source))
                    .expect("Failed to open image");
                let img = img.as_rgba8().expect("Failed to convert image to rgba8");
                let dimensions = img.dimensions();
                let bytes = img
                    .pixels()
                    .flat_map(|it| it.data.iter().map(|it| *it))
                    .collect::<Vec<u8>>();
                images.push(InputSprite {
                    dimensions,
                    bytes: bytes.clone(),
                });
            }
            let sprite_sheet = sheep::pack::<SimplePacker>(images, 4);
            let meta = sheep::encode::<AmethystFormat>(&sprite_sheet, ());
            let texture_path = base_path.with_extension("png");
            let mut buffer = Vec::new();
            let encoder = PNGEncoder::new(&mut buffer);
            encoder.encode(&sprite_sheet.bytes, sprite_sheet.dimensions.0, sprite_sheet.dimensions.1, ColorType::RGBA(8)).expect("Could not encode spritesheet as png");


            let sprites: Vec<_> = meta
                .sprites
                .iter()
                .map(|s| SpritePosition {
                    x: s.x as u32,
                    y: s.y as u32,
                    width: s.width as u32,
                    height: s.height as u32,
                    offsets: s.offsets,
                    flip_horizontal: false,
                    flip_vertical: false,
                })
                .collect();

            let sprite_sheet = SpriteSheetPrefab::Sheet {
                texture: TexturePrefab::File(
                    map_prefix.join(&texture_path).to_str().unwrap().to_string(),
                    (
                        "IMAGE".to_string(),
                        Box::new(ImageFormat::default()).0.sampler_info,
                    ),
                ),
                sprites: vec![Sprites::List(SpriteList {
                    texture_width: meta.texture_width as u32,
                    texture_height: meta.texture_height as u32,
                    sprites: sprites,
                })],
                name: Some(format!("map_sprite_sheet_{}", i)),
            };
            spritesheets.push((MapFile::Data(texture_path, buffer), sprite_sheet));
        }
    }
    spritesheets
}

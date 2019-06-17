mod prefab_proxies;

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
        formats::texture::ImageFormat as RealImageFormat,
        sprite::{
            prefab::{SpriteSheetPrefab as RealSpriteSheetPrefab, SpriteRenderPrefab as RealSpriteRenderPrefab},
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


pub use prefab_proxies::*;

use tiled::{Object, Map, parse_file, ObjectGroup};

pub struct SpriteContext {
    pub sprite_sheet: Option<SpriteSheetPrefab>,
    pub name: String,
    pub sprite_sheet_id: u32,
    pub sprite_id: u32,
    pub sprite_width: u32,
    pub sprite_height: u32,
}

impl SpriteContext {
    pub fn from_gid(gid: u32, map: &Map, gid_map: &HashMap<usize, (usize, usize)>, sprite_sheets: &Vec<SpriteSheetPrefab>, used_sprite_sheets: &HashSet<u32>) -> Option<Self> {
        if gid != 0 {
            let mut keys: Vec<_> = gid_map.keys().collect();
            keys.sort();
            let (sprite_sheet_id, sprite_id) = &gid_map[&(gid as usize)];
            let map_tileset = &map.tilesets[*sprite_sheet_id];
            let sprite_sheet = &sprite_sheets[*sprite_sheet_id as usize];
            let name = match sprite_sheet {
                SpriteSheetPrefab::Sheet { name, .. } => name,
                _ => panic!(),
            };
            let sprite_sheet = if used_sprite_sheets.contains(&(*sprite_sheet_id as u32)) {
                None
            } else {
                Some(sprite_sheet.clone())
            };
            Some(SpriteContext {
                sprite_sheet,
                name: name.as_ref().unwrap().clone(),
                sprite_sheet_id: *sprite_sheet_id as u32,
                sprite_id: *sprite_id as u32,
                sprite_width: map_tileset.tile_width,
                sprite_height: map_tileset.tile_height,
            })
        } else {
            None
        }
    }
}

pub struct MapContext<P> {
    pub map: Map,
    pub sprite_sheets: Vec<SpriteSheetPrefab>,
    pub used_sprite_sheets: HashSet<u32>,
    pub gid_map: HashMap<usize, (usize, usize)>,
    pub entities: Vec<PrefabEntity<P>>,
}

pub enum MapFile {
    Path(PathBuf, PathBuf),
    Data(PathBuf, Vec<u8>),
}
pub trait TiledConverter<'s, P> 
    where P: PrefabData<'s> {
        type PrefabProxy: Serialize;

        fn convert_tile(ctx: &Option<SpriteContext>, x: f32, y: f32, layer: usize) -> Option<Self::PrefabProxy>;
        fn convert_object(ctx: &Option<SpriteContext>, layer: usize, object: &Object) -> Option<Self::PrefabProxy>;

        fn base_convert_object_group(map_context: &mut MapContext<Self::PrefabProxy>, group_id: usize) {
            let group = &map_context.map.object_groups[group_id];
            for object in &group.objects {
                let ctx = SpriteContext::from_gid(object.gid, &map_context.map, &map_context.gid_map, &map_context.sprite_sheets, &map_context.used_sprite_sheets);
                if let Some(object) = Self::convert_object(&ctx, group_id, object) {
                    if let Some(ctx) = ctx {
                        map_context.used_sprite_sheets.insert(ctx.sprite_sheet_id);
                    }
                    map_context.entities.push(
                        PrefabEntity { data: Some(object) }
                    );
                }
            }
        }

        fn convert_object_group(map_context: &mut MapContext<Self::PrefabProxy>, group_id: usize) {
            Self::base_convert_object_group(map_context, group_id);
        }

        fn from_map(input: &Path, map_prefix: &Path) -> MapPrefab<P, Self::PrefabProxy> {
            let input_dir = input.parent().unwrap();
            let map = parse_file(input).unwrap();

            let (mut sprite_sheets, gid_map) = sprite_sheets_from_tilesets(&map, &input_dir, map_prefix);
            let (mut sprite_files, sprite_sheets): (Vec<MapFile>, Vec<SpriteSheetPrefab>) = sprite_sheets.drain(..).unzip();

            let mut map_context = MapContext {
                map,
                sprite_sheets,
                used_sprite_sheets: HashSet::new(),
                gid_map,
                entities: Vec::new(),
            };

            for (z, layer) in map_context.map.layers.iter().enumerate() {
                for (y, row) in layer.tiles.iter().enumerate() {
                    for (x, gid) in row.iter().enumerate() {
                        let ctx = SpriteContext::from_gid(*gid, &map_context.map, &map_context.gid_map, &map_context.sprite_sheets, &map_context.used_sprite_sheets);
                        if let Some(tile) = Self::convert_tile(&ctx, x as f32 * map_context.map.tile_width as f32 + map_context.map.tile_width as f32 / 2.0, y as f32 * -(map_context.map.tile_height as f32) - map_context.map.tile_height as f32 / 2.0, z) {
                            if let Some(ctx) = ctx {
                                map_context.used_sprite_sheets.insert(ctx.sprite_sheet_id);
                            }
                            map_context.entities.push(
                                PrefabEntity { data: Some(tile) }
                            );
                        }
                    }
                }
            }

            for group_id in 0..map_context.map.object_groups.len() {
               Self::convert_object_group(&mut map_context, group_id);
            }


            let map = Prefab { entities: map_context.entities }; 
            let buffer = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
                .expect("Failed to encode map prefab file");
            sprite_files.push(MapFile::Data(PathBuf::from("map.ron"), buffer.into_bytes()));

            MapPrefab::new(sprite_files)
        }
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


pub fn sprite_sheets_from_tilesets(map: &Map, input_dir: &Path, map_prefix: &Path) -> (Vec<(MapFile, SpriteSheetPrefab)>, HashMap<usize, (usize, usize)>) {
    let mut spritesheets = Vec::new();
    let mut gid_to_sprite = HashMap::new();
    for (i, tileset) in map.tilesets.iter().enumerate() {
        let base_path = PathBuf::from(format!("sprite_sheet_{}", i));

        if !tileset.images.is_empty() {
            let img = &tileset.images[0];
            let texture_path = base_path.with_extension("png");

            let mut sprites = vec![];
            let tileset_sprite_columns = img.width / tileset.tile_width as i32;
            let tileset_sprite_rows = img.height / tileset.tile_height as i32;


            for y in (0..tileset_sprite_rows).rev() {
                for x in 0..tileset_sprite_columns {
                    gid_to_sprite.insert(sprites.len() + tileset.first_gid as usize, (spritesheets.len(), sprites.len()));
                    sprites.push(SpritePosition {
                        y: img.height as u32 - (y as u32 + 1) * tileset.tile_height as u32,
                        x: x as u32 * tileset.tile_width as u32,
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
                        ImageFormat {
                            sampler_info: RealImageFormat::default().0.sampler_info,
                        },
                    ),
                ),
                sprites: vec![Sprites::List(SpriteList {
                    texture_width: img.width as u32,
                    texture_height: img.height as u32,
                    sprites: sprites,
                })],
                name: Some(format!("{:?}_sprite_sheet_{}", map_prefix, i)),
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
                gid_to_sprite.insert(tileset.first_gid as usize + tile.id as usize, (spritesheets.len(), images.len()));
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
                        ImageFormat {
                            sampler_info: RealImageFormat::default().0.sampler_info,
                        },
                    ),
                ),
                sprites: vec![Sprites::List(SpriteList {
                    texture_width: meta.texture_width as u32,
                    texture_height: meta.texture_height as u32,
                    sprites: sprites,
                })],
                name: Some(format!("{:?}_sprite_sheet_{}", map_prefix, i)),
            };
            spritesheets.push((MapFile::Data(texture_path, buffer), sprite_sheet));
        }
    }
    (spritesheets, gid_to_sprite)
}

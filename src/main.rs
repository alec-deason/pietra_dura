use amethyst::{
    core::Transform,
    renderer::{
        formats::texture::ImageFormat,
        sprite::prefab::SpriteSheetReference,
        sprite::{SpriteList, SpritePosition, Sprites},
    },
};
use sheep::{AmethystFormat, InputSprite, SimplePacker};
use std::collections::HashSet;
use std::fs::{copy, create_dir_all};
use std::{fs::File, io::prelude::*};
use structopt::StructOpt;
use tiled::parse_file;

use tiled_preprocessor::{
    LevelPrefab, Prefab, PrefabEntity, SpriteRenderPrefab, SpriteSheetPrefab, TexturePrefab, Tile,
};

#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(parse(from_os_str))]
    map: std::path::PathBuf,
    #[structopt(parse(from_os_str))]
    output: std::path::PathBuf,
}

fn main() {
    let args = Cli::from_args();

    let map = parse_file(&args.map).unwrap();
    let map_name = args.map.file_stem().unwrap();
    let input_dir = args.map.parent().unwrap();
    let target_dir = args.output.join(map_name);
    create_dir_all(&target_dir).unwrap();

    let mut spritesheets = Vec::new();
    for (i, tileset) in map.tilesets.iter().enumerate() {
        let base_path = target_dir.join(format!("sprite_sheet_{}", i));
        if !tileset.images.is_empty() {
            let img = &tileset.images[0];
            let texture_path = base_path.with_extension("png");
            copy(input_dir.join(&img.source), &texture_path).unwrap();

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
                    texture_path.to_str().unwrap().to_string(),
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
                name: Some(format!("level_spritesheet_{}", i)),
            };
            spritesheets.push(sprite_sheet);
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
            let outbuf = image::RgbaImage::from_vec(
                sprite_sheet.dimensions.0,
                sprite_sheet.dimensions.1,
                sprite_sheet.bytes,
            )
            .expect("Failed to construct image from sprite sheet bytes");
            let texture_path = base_path.with_extension("png");
            outbuf.save(&texture_path).expect("Failed to save image");

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
                    texture_path.to_str().unwrap().to_string(),
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
                name: Some(format!("level_spritesheet_{}", i)),
            };
            spritesheets.push(sprite_sheet);
        }
    }

    let mut used_spritesheets = HashSet::new();
    let mut tiles = Vec::new();
    for (z, layer) in map.layers.iter().enumerate() {
        let z = z as f32 / 100.0;
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
                    Some(spritesheets[sprite_sheet_id].clone())
                };
                let render = SpriteRenderPrefab {
                    sheet: Some(SpriteSheetReference::Name(format!(
                        "level_spritesheet_{}",
                        sprite_sheet_id
                    ))),
                    sprite_number: sprite_id,
                };
                let mut transform = Transform::default();
                transform.set_translation_xyz(
                    x as f32 * map.tile_width as f32 + map.tile_width as f32 / 2.0,
                    -(y as f32) * map.tile_height as f32 - map.tile_height as f32 / 2.0,
                    z,
                );
                tiles.push(PrefabEntity {
                    data: Some(LevelPrefab {
                        sheet: sprite_sheet,
                        render: Some(render),
                        transform: Some(transform),
                        detail: Tile,
                    }),
                });
            }
        }
    }

    for group in map.object_groups.iter() {
        for object in &group.objects {
            match object.obj_type.as_str() {
                object_type => {
                    eprintln!(
                        "Found object of type '{}' but objects not supported yet",
                        object_type
                    );
                }
            }
        }
    }

    let level = Prefab { entities: tiles };
    let mut target_file =
        File::create(target_dir.join("level.ron")).expect("Failed to create meta file");
    println!("creating level ron");
    let target_str = ron::ser::to_string_pretty(&level, ron::ser::PrettyConfig::default())
        .expect("Failed to encode level file");
    println!("done creating level ron");
    target_file
        .write_all(target_str.as_bytes())
        .expect("Failed to write target level file");
}

use amethyst::{
    assets::PrefabData,
    core::math::{Point2, Quaternion, Unit},
    core::bundle::SystemBundle,
    core::transform::Transform,
    ecs::prelude::*,
    ecs::{Entity, Join, ReadStorage, System, WriteStorage},
    error::Error,
};
use serde::{Deserialize, Serialize};
use specs_derive::Component;

use nalgebra::Vector2;
use ncollide2d::{
    shape::{Ball, Cuboid, ConvexPolygon, ShapeHandle},
    world::CollisionGroups,
};
use nphysics2d::{
    material::{BasicMaterial, MaterialHandle},
    object::{BodyHandle, ColliderDesc, RigidBodyDesc},
    world::World as PhysicsWorld,
};

pub const PHYSICS_SCALE:f32 = 10.0;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollisionGroupPrefab<CollisionTypeEnum>
    where CollisionTypeEnum: Into<usize> + Copy {
    pub membership: Vec<CollisionTypeEnum>,
    pub whitelist: Vec<CollisionTypeEnum>,
    pub blacklist: Vec<CollisionTypeEnum>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ShapePrefab {
    Ball { radius: f32 },
    Rect { width: f32, height: f32 },
    Polygon { points: Vec<Point2<f32>>},
}

impl ShapePrefab {
    fn size(&self) -> (f32, f32) {
        match self {
            ShapePrefab::Ball { radius } => {
                (*radius*2.0, *radius*2.0)
            },
            ShapePrefab::Rect { width, height } => {
                (*width, *height)
            },
            ShapePrefab::Polygon { points } => {
                (0.0, 0.0)
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColliderPrefab<CollisionTypeEnum> 
    where CollisionTypeEnum: Into<usize> + Copy {
    pub shape: ShapePrefab,
    pub density: f32,
    pub restitution: f32,
    pub friction: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub is_sensor: bool,
    pub collision_group: CollisionGroupPrefab<CollisionTypeEnum>,
    pub location: Option<(f32, f32)>,
}

#[derive(Component, Default, Copy, Clone, Debug)]
pub struct InitialPosition {
    // This is dumb as shit but it lets me communicate a location
    // without needing Transform so I don't have to worry about
    // double borrows
    pub x: f32,
    pub y: f32
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PhysicsEntityPrefab<CollisionTypeEnum>
    where CollisionTypeEnum: Into<usize> + Copy {
    pub colliders: Vec<ColliderPrefab<CollisionTypeEnum>>,
    pub gravity_enabled: bool,
    pub no_rotate: bool,
    pub collider_only: bool,
    pub location: Option<(f32, f32)>,
}   

impl<CollisionTypeEnum> PhysicsEntityPrefab<CollisionTypeEnum>
    where CollisionTypeEnum: Into<usize> + Copy {
    pub fn from_shape(shape: ShapePrefab, location: Option<(f32, f32)>, collider_only: bool, collider_location: Option<(f32, f32)>) -> Self {
        PhysicsEntityPrefab {
            colliders: vec![ColliderPrefab {
                shape: shape,
                density: 1.0,
                restitution: 0.8,
                friction: 0.5,
                offset_x: 0.0,
                offset_y: 0.0,
                is_sensor: false,
                collision_group: CollisionGroupPrefab {
                    membership: vec![],
                    whitelist: vec![],
                    blacklist: vec![],
                },
                location: collider_location,
            }],
            collider_only: collider_only,
            gravity_enabled: true,
            no_rotate: false,
            location: location,
        }
    }
}

impl<'s, CollisionTypeEnum> PrefabData<'s> for PhysicsEntityPrefab<CollisionTypeEnum>
    where CollisionTypeEnum: Into<usize> + Copy {
    type SystemData = (
        WriteStorage<'s, PhysicsEntity>,
        WriteExpect<'s, PhysicsWorld<f32>>,
        ReadStorage<'s, InitialPosition>,
        WriteStorage<'s, NoRotate>,
        );
    type Result = ();

    fn add_to_entity(
        &self,
        entity: Entity,
        data: &mut Self::SystemData,
        _: &[Entity],
        _: &[Entity],
        ) -> Result<(), Error> {
        let physics_entities = &mut data.0;
        let physics_world = &mut data.1;
        let starting_locations = &mut data.2;
        let no_rotates = &mut data.3;
        let mut collider_descs = Vec::with_capacity(self.colliders.len());
        for collider in &self.colliders {
            let shape = match &collider.shape {
                ShapePrefab::Ball { radius } => ShapeHandle::new(Ball::new(*radius*PHYSICS_SCALE)),
                ShapePrefab::Rect { width, height } => {
                    ShapeHandle::new(Cuboid::new(Vector2::new((*width*PHYSICS_SCALE) / 2.0, (*height*PHYSICS_SCALE) / 2.0)))
                },
                ShapePrefab::Polygon { points } => {
                    let points: Vec<_> = points.iter().map(|p| p * PHYSICS_SCALE).collect();
                    ShapeHandle::new(
                        ConvexPolygon::try_from_points(&points).unwrap()
                    )
                },
            };
            let mut group = CollisionGroups::new();
            group.set_membership(
                &collider
                .collision_group
                .membership
                .iter()
                .map(|g| (*g).into())
                .collect::<Vec<usize>>()
                );
            group.set_whitelist(
                &collider
                .collision_group
                .whitelist
                .iter()
                .map(|g| (*g).into())
                .collect::<Vec<usize>>()
                );
            group.set_blacklist(
                &collider
                .collision_group
                .blacklist
                .iter()
                .map(|g| (*g).into())
                .collect::<Vec<usize>>()
                );
            let mut collider_desc = ColliderDesc::new(shape)
                .collision_groups(group)
                .material(MaterialHandle::new(BasicMaterial {
                    friction: collider.friction,
                    restitution: collider.restitution,
                    ..BasicMaterial::default()
                }))
            .sensor(collider.is_sensor)
                .density(collider.density);
            if let Some((x, y)) = collider.location {
                collider_desc.set_translation(Vector2::new(
                        x*PHYSICS_SCALE,
                        y*PHYSICS_SCALE,
                ));
            }
            collider_desc.set_user_data(Some(Box::new(entity)));
            if self.collider_only {
                collider_desc.build(physics_world);
            } else {
                collider_descs.push(collider_desc);
            }
        }
        if !self.collider_only {
            let mut rb_desc = RigidBodyDesc::new();
            for collider_desc in &collider_descs {
                rb_desc.add_collider(collider_desc);
            }

            let (x, y) = starting_locations
                .get(entity)
                .map(|l| (l.x, l.y))
                .unwrap_or(self.location.unwrap_or((0.0, 0.0)));

            let body = rb_desc
                .gravity_enabled(self.gravity_enabled)
                .set_translation(Vector2::new(x*PHYSICS_SCALE, y*PHYSICS_SCALE))
                .build(physics_world);
            physics_entities
                .insert(
                    entity,
                    PhysicsEntity {
                        handle: body.handle(),
                    },
                    )
                .unwrap();
            if self.no_rotate {
                no_rotates
                    .insert(
                        entity,
                        NoRotate,
                        )
                    .unwrap();
            }
        }
        Ok(())
    }
}


#[derive(Default, Debug, Copy, Clone, Component)]
pub struct NoRotate;

#[derive(Component)]
pub struct PhysicsEntity {
    pub handle: BodyHandle,
}

struct PhysicalSimulationSystem;
impl<'s> System<'s> for PhysicalSimulationSystem {
    type SystemData = WriteExpect<'s, PhysicsWorld<f32>>;

    fn run(&mut self, mut physics_world: Self::SystemData) {
        physics_world.step();
    }
}

struct PhysicalPlacementSystem;
impl<'s> System<'s> for PhysicalPlacementSystem {
    type SystemData = (
        Entities<'s>,
        WriteStorage<'s, Transform>,
        WriteExpect<'s, PhysicsWorld<f32>>,
        ReadStorage<'s, PhysicsEntity>,
        ReadStorage<'s, NoRotate>,
    );

    fn run(&mut self, (entities, mut transforms, physics_world, physics_entities, no_rotates): Self::SystemData) {
        for (e, physics_entity, transform) in (&entities, &physics_entities, &mut transforms).join() {
            if let Some(body) = physics_world.rigid_body(physics_entity.handle) {
                let position = body.position();

                transform.set_translation_x((position.translation.x/PHYSICS_SCALE).floor());
                transform.set_translation_y((position.translation.y/PHYSICS_SCALE).floor());
                if !no_rotates.contains(e) {
                    let rot = position.rotation.angle() * 0.5;
                    transform.set_rotation(Unit::new_normalize(Quaternion::new(
                        rot.cos(),
                        0.0,
                        0.0, 
                        rot.sin(),
                    )));
                }
            }
        }
    }
}

#[derive(Default)]
pub struct PhysicsBundle;
impl PhysicsBundle {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a, 'b> SystemBundle<'a, 'b> for PhysicsBundle {
    fn build(self, builder: &mut DispatcherBuilder<'a, 'b>) -> Result<(), Error> {
        builder.add(PhysicalPlacementSystem, "pietra_dura_physical_placement_system", &[]);
        builder.add(PhysicalSimulationSystem, "pietra_dura_physical_simulation_system", &[]);
        Ok(())
    }
}

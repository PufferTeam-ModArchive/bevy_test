use crate::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn collision_info(&self, other: &AABB) -> (Vec3, f32) {
        let overlap_x1 = self.max.x - other.min.x;
        let overlap_x2 = other.max.x - self.min.x;
        let overlap_y1 = self.max.y - other.min.y;
        let overlap_y2 = other.max.y - self.min.y;
        let overlap_z1 = self.max.z - other.min.z;
        let overlap_z2 = other.max.z - self.min.z;

        let x_pen = overlap_x1.min(overlap_x2);
        let y_pen = overlap_y1.min(overlap_y2);
        let z_pen = overlap_z1.min(overlap_z2);

        let self_center = self.center();
        let other_center = other.center();

        if x_pen < y_pen && x_pen < z_pen {
            (
                if self_center.x < other_center.x {
                    Vec3::X
                } else {
                    -Vec3::X
                },
                x_pen,
            )
        } else if y_pen < z_pen {
            (
                if self_center.y < other_center.y {
                    Vec3::Y
                } else {
                    -Vec3::Y
                },
                y_pen,
            )
        } else {
            (
                if self_center.z < other_center.z {
                    Vec3::Z
                } else {
                    -Vec3::Z
                },
                z_pen,
            )
        }
    }
}

pub fn update_collision(
    query: Query<(&Transform, &AABB), Without<Player>>,
    player: Single<(&mut Transform, &mut PlayerInfo, &AABB, &mut PhysicalTranslation,), With<Player>>,
) {
    let (mut player_transform, mut player_info, player_aabb, mut translation) = player.into_inner();

    player_info.on_ground = false;
    for (transform, aabb) in &query {
        let player_world = AABB::new(
            translation.0 + player_aabb.min,
            translation.0 + player_aabb.max,
        );
        
        let world_aabb = AABB::new(
            transform.translation + aabb.min,
            transform.translation + aabb.max,
        );

        if player_world.intersects(&world_aabb) {
            let (normal, penetration) = world_aabb.collision_info(&player_world);

            translation.0 += normal * penetration;
            if normal == Vec3::NEG_Y || normal == Vec3::Y {
                player_info.on_ground = true;
            }
        }
    }
}

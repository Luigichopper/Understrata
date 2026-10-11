//! Kinematic Capsule Character Controller vs continuous SDF terrain (§7.1, §7.2).
//! Implements MOV-001 (walk, jump, slope, step, slide), MOV-002 (depenetration), MOV-003 (sprint, crouch, coyote, buffer).
//! Pure Rust: NO Bevy dependencies.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use us_core::ChunkPos;
use us_voxel::{capsule_overlap, gradient_at_world, raycast_terrain, sdf_at_world, TerrainChunk};

/// Character controller physics and tuning configuration (§7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterControllerConfig {
    pub standing_height: f32,
    pub crouching_height: f32,
    pub radius: f32,
    pub walk_speed: f32,
    pub sprint_speed: f32,
    pub crouch_speed: f32,
    pub gravity: f32,
    pub jump_impulse: f32,
    pub air_control: f32,
    pub step_up_height: f32,
    pub ground_snap_dist: f32,
    pub coyote_time: f32,
    pub jump_buffer: f32,
    pub max_slope_cos: f32,
    pub max_depenetration_per_tick: f32,
}

impl Default for CharacterControllerConfig {
    fn default() -> Self {
        Self {
            standing_height: 1.8,
            crouching_height: 1.2,
            radius: 0.35,
            walk_speed: 4.3,
            sprint_speed: 6.5,
            crouch_speed: 2.0,
            gravity: 24.0,
            // v = sqrt(2 * g * h) = sqrt(2 * 24.0 * 1.15) ≈ 7.43 m/s
            jump_impulse: 7.43,
            air_control: 0.30,
            step_up_height: 0.60,
            ground_snap_dist: 0.35,
            coyote_time: 0.100,
            jump_buffer: 0.100,
            // cos(65°) ≈ 0.4226183 - player only slides on very steep hills (> 65°)
            max_slope_cos: 0.4226183,
            // MOV-002: no teleport > 1.5 m
            max_depenetration_per_tick: 1.5,
        }
    }
}

/// Dynamic character physics state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterState {
    /// Bottom center / feet position of the capsule.
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_crouching: bool,
    pub current_height: f32,
    pub coyote_timer: f32,
    pub jump_buffer_timer: f32,
    pub stuck_timer: f32,
}

impl CharacterState {
    pub fn new(spawn_pos: Vec3) -> Self {
        Self {
            position: spawn_pos,
            velocity: Vec3::ZERO,
            on_ground: false,
            is_crouching: false,
            current_height: 1.8,
            coyote_timer: 0.0,
            jump_buffer_timer: 0.0,
            stuck_timer: 0.0,
        }
    }

    /// Returns the eye position in world space.
    pub fn eye_position(&self) -> Vec3 {
        let eye_offset = if self.is_crouching { 1.05 } else { 1.65 };
        self.position + Vec3::new(0.0, eye_offset, 0.0)
    }
}

/// Per-tick player inputs passed to the controller.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CharacterInput {
    /// Horizontal movement direction: X = strafe right/left, Y = forward/backward.
    pub move_direction: Vec2,
    pub yaw: f32,
    pub jump: bool,
    pub sprint: bool,
    pub crouch: bool,
}

/// Evaluates post-edit depenetration (MOV-002) to eject the player to the nearest free pose.
/// Guaranteed to never teleport greater than `max_depenetration_per_tick` (1.5 m).
pub fn depenetrate_player(
    chunks: &HashMap<ChunkPos, TerrainChunk>,
    position: &mut Vec3,
    height: f32,
    radius: f32,
    max_ejection: f32,
) -> bool {
    let mut ejected = false;
    let mut total_push = 0.0f32;

    for _ in 0..4 {
        if let Some((normal, penetration)) = capsule_overlap(chunks, *position, height, radius) {
            ejected = true;
            let step = penetration.min(max_ejection - total_push);
            if step <= 1e-4 {
                break;
            }
            *position += normal * step;
            total_push += step;
            if total_push >= max_ejection {
                break;
            }
        } else {
            break;
        }
    }

    ejected
}

/// Executes one kinematic simulation step of the character controller against the terrain SDF.
pub fn step_character(
    config: &CharacterControllerConfig,
    state: &mut CharacterState,
    input: &CharacterInput,
    chunks: &HashMap<ChunkPos, TerrainChunk>,
    dt: f32,
) {
    if dt <= 0.0 {
        return;
    }

    // 1. Crouch / Un-crouch logic (MOV-003)
    if input.crouch {
        state.is_crouching = true;
        state.current_height = config.crouching_height;
    } else if state.is_crouching {
        // Can't un-crouch under low ceiling: probe upward clearance
        let blocked = capsule_overlap(
            chunks,
            state.position,
            config.standing_height,
            config.radius,
        );
        if blocked.is_none() {
            state.is_crouching = false;
            state.current_height = config.standing_height;
        }
    }

    // 2. Timers for coyote time & jump buffering (MOV-003)
    if state.on_ground {
        state.coyote_timer = config.coyote_time;
    } else {
        state.coyote_timer = (state.coyote_timer - dt).max(0.0);
    }

    if input.jump {
        state.jump_buffer_timer = config.jump_buffer;
    } else {
        state.jump_buffer_timer = (state.jump_buffer_timer - dt).max(0.0);
    }

    // 3. Compute desired horizontal velocity from input
    let speed = if state.is_crouching {
        config.crouch_speed
    } else if input.sprint {
        config.sprint_speed
    } else {
        config.walk_speed
    };

    let rot = glam::Quat::from_rotation_y(input.yaw);
    let raw_dir = Vec3::new(input.move_direction.x, 0.0, -input.move_direction.y);
    let world_dir = if raw_dir.length_squared() > 1e-4 {
        rot * raw_dir.normalize()
    } else {
        Vec3::ZERO
    };
    let desired_vel_h = world_dir * speed;

    // 4. Integrate horizontal acceleration
    if state.on_ground {
        state.velocity.x = desired_vel_h.x;
        state.velocity.z = desired_vel_h.z;
    } else {
        // Air control: gradual steering towards desired
        let air_factor = (config.air_control * 15.0 * dt).min(1.0);
        state.velocity.x = state.velocity.x * (1.0 - air_factor) + desired_vel_h.x * air_factor;
        state.velocity.z = state.velocity.z * (1.0 - air_factor) + desired_vel_h.z * air_factor;
    }

    // 5. Jump logic
    if state.coyote_timer > 0.0 && state.jump_buffer_timer > 0.0 {
        state.velocity.y = config.jump_impulse;
        state.coyote_timer = 0.0;
        state.jump_buffer_timer = 0.0;
        state.on_ground = false;
    }

    // 6. Gravity integration
    if state.on_ground {
        if state.velocity.y > 0.0 {
            // Jumping: apply gravity
            state.velocity.y -= config.gravity * dt;
        } else {
            // Grounded: slight downward stick velocity to smoothly adhere to slopes without vertical vibration
            state.velocity.y = -0.5;
        }
    } else {
        // Airborne: full gravity acceleration
        state.velocity.y -= config.gravity * dt;
        state.velocity.y = state.velocity.y.max(-60.0);
    }

    // 7. Swept collision detection & move-and-slide
    let total_disp = state.velocity * dt;
    let disp_len = total_disp.length();
    let max_substep = config.radius * 0.4;
    let substeps = ((disp_len / max_substep).ceil() as usize).max(1);
    let sub_dt = dt / substeps as f32;

    let initial_pos = state.position;
    let mut grounded = false;

    for _ in 0..substeps {
        let sub_disp = state.velocity * sub_dt;
        if sub_disp.length_squared() < 1e-8 {
            continue;
        }

        // Step-up check (MOV-001): when moving horizontally, test if lifting capsule clears a low curb
        let h_disp = Vec3::new(sub_disp.x, 0.0, sub_disp.z);
        if state.on_ground && h_disp.length_squared() > 1e-6 {
            let next_ground_probe = state.position + h_disp;
            if let Some((normal, _pen)) = capsule_overlap(
                chunks,
                next_ground_probe,
                state.current_height,
                config.radius,
            ) {
                if normal.y < config.max_slope_cos {
                    // Wall or curb encountered: test if lifting capsule by step_up_height clears it
                    let high_pos = state.position + Vec3::Y * config.step_up_height + h_disp;
                    if capsule_overlap(chunks, high_pos, state.current_height, config.radius)
                        .is_none()
                    {
                        // Raycast downward from high_pos to find the top surface of the step/curb
                        if let Some(hit) =
                            raycast_terrain(chunks, high_pos, Vec3::NEG_Y, config.step_up_height)
                        {
                            if hit.normal.y >= config.max_slope_cos {
                                let step_pos =
                                    Vec3::new(high_pos.x, hit.position.y + 0.005, high_pos.z);
                                // Ensure stepped position does not collide with ceilings or overhangs
                                if capsule_overlap(
                                    chunks,
                                    step_pos,
                                    state.current_height,
                                    config.radius,
                                )
                                .is_none()
                                {
                                    state.position = step_pos;
                                    grounded = true;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }

        state.position += sub_disp;

        // Slide iterations (iterate ≤ 4 times per §7.1)
        for _ in 0..4 {
            if let Some((normal, penetration)) =
                capsule_overlap(chunks, state.position, state.current_height, config.radius)
            {
                // Depenetrate along normal (exact penetration, no artificial pop)
                state.position += normal * penetration.min(config.max_depenetration_per_tick);

                // Project velocity onto tangent plane: slide
                let v_dot_n = state.velocity.dot(normal);
                if v_dot_n < 0.0 {
                    state.velocity -= normal * v_dot_n;
                }

                // Check slope walkability (§7.1)
                if normal.y >= config.max_slope_cos {
                    grounded = true;
                    state.velocity.y = state.velocity.y.max(0.0);
                    if desired_vel_h.length_squared() < 1e-4 {
                        state.velocity.x = 0.0;
                        state.velocity.z = 0.0;
                    }
                } else if normal.y < -0.1 {
                    // Ceiling or overhang hit: cancel upward velocity and jump buffer
                    state.velocity.y = state.velocity.y.min(0.0);
                    state.jump_buffer_timer = 0.0;
                } else if normal.y > 0.05 {
                    // Very steep hill: unwalkable (> 65°); actively slide down the slope tangent
                    state.velocity.y = state.velocity.y.min(0.0);
                    let downhill_h = Vec3::new(normal.x, 0.0, normal.z);
                    let h_len_sq = downhill_h.length_squared();
                    if h_len_sq > 1e-4 {
                        let h_len = h_len_sq.sqrt();
                        let downhill_dir = Vec3::new(
                            normal.x / h_len * normal.y,
                            -h_len,
                            normal.z / h_len * normal.y,
                        );
                        // Cancel any uphill velocity attempting to climb unwalkable slope
                        let uphill_comp = state.velocity.dot(downhill_dir);
                        if uphill_comp < 0.0 {
                            state.velocity -= downhill_dir * uphill_comp;
                        }
                        // Accelerate downward along the slope tangent
                        let slide_accel = config.gravity * h_len;
                        state.velocity += downhill_dir * (slide_accel * sub_dt);
                    }
                }
            } else {
                break;
            }
        }
    }

    // 8. Ground probe & snap (strictly downward probe ≤ 0.3 m when descending slopes)
    if !grounded && state.velocity.y <= 0.0 {
        let sample_steps = 6;
        for i in 1..=sample_steps {
            let drop = (i as f32 / sample_steps as f32) * config.ground_snap_dist;
            let probe_p = state.position - Vec3::Y * drop;
            let sdf = sdf_at_world(chunks, probe_p);
            if sdf <= 0.0 {
                let prev_y = state.position.y
                    - ((i - 1) as f32 / sample_steps as f32) * config.ground_snap_dist;
                let prev_sdf = sdf_at_world(chunks, Vec3::new(probe_p.x, prev_y, probe_p.z));
                let t = if (prev_sdf - sdf).abs() > 1e-5 {
                    prev_sdf / (prev_sdf - sdf)
                } else {
                    1.0
                };
                let surface_y = prev_y + t * (probe_p.y - prev_y);
                let normal = gradient_at_world(chunks, Vec3::new(probe_p.x, surface_y, probe_p.z));
                if normal.y >= config.max_slope_cos {
                    state.position.y = surface_y;
                    state.velocity.y = 0.0;
                    if desired_vel_h.length_squared() < 1e-4 {
                        state.velocity.x = 0.0;
                        state.velocity.z = 0.0;
                    }
                    grounded = true;
                }
                break;
            }
        }
    }

    state.on_ground = grounded;

    // 9. Post-edit depenetration safeguard (MOV-002)
    depenetrate_player(
        chunks,
        &mut state.position,
        state.current_height,
        config.radius,
        config.max_depenetration_per_tick,
    );

    // 10. Stuck tracking
    let moved_dist = (state.position - initial_pos).length();
    if input.move_direction.length_squared() > 0.1 && moved_dist < 0.005 {
        state.stuck_timer += dt;
        // Automatic small nudge/depenetration if stuck > 0.5s
        if state.stuck_timer > 0.5 {
            let nudge = gradient_at_world(chunks, state.position + Vec3::Y * config.radius);
            state.position += nudge * 0.05;
        }
    } else {
        state.stuck_timer = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_core::{ChunkPos, Rng64};
    use us_voxel::sdf_at_world;

    /// Generates a test terrain chunk with a flat floor at y = 16.0.
    fn create_flat_terrain() -> HashMap<ChunkPos, TerrainChunk> {
        let mut chunks = HashMap::new();
        let cpos = ChunkPos::new(0, 0, 0);
        let mut chunk = TerrainChunk::new_air(cpos);

        // Fill with linear metric SDF with surface at y = 16.0
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let sdf = (y as f32) - 16.0;
                    chunk.set_sdf(x, y, z, sdf);
                    chunk.set_material(x, y, z, if sdf <= 0.0 { 1 } else { 0 });
                }
            }
        }
        chunks.insert(cpos, chunk);
        chunks
    }

    #[test]
    fn test_mov_001_walk_and_gravity_on_flat_ground() {
        let config = CharacterControllerConfig::default();
        let chunks = create_flat_terrain();
        let mut state = CharacterState::new(Vec3::new(16.0, 18.0, 16.0));

        let input = CharacterInput {
            move_direction: Vec2::new(0.0, 1.0), // Forward
            yaw: 0.0,
            jump: false,
            sprint: false,
            crouch: false,
        };

        // Fall to ground
        for _ in 0..60 {
            step_character(&config, &mut state, &input, &chunks, 1.0 / 60.0);
        }

        assert!(state.on_ground, "Player must land on solid ground");
        assert!(
            state.position.y >= 15.0 && state.position.y <= 16.5,
            "Player y position must settle near surface: {}",
            state.position.y
        );

        // Move forward while grounded
        let start_z = state.position.z;
        for _ in 0..30 {
            step_character(&config, &mut state, &input, &chunks, 1.0 / 60.0);
        }
        assert!(
            state.position.z < start_z,
            "Forward input must move player along -Z"
        );
        assert!(state.on_ground, "Player remains grounded while walking");
    }

    #[test]
    fn test_mov_002_post_edit_depenetration() {
        let config = CharacterControllerConfig::default();
        let mut chunks = create_flat_terrain();
        let mut state = CharacterState::new(Vec3::new(16.0, 16.0, 16.0));

        // Simulate placing a solid block directly onto the player
        if let Some(chunk) = chunks.get_mut(&ChunkPos::new(0, 0, 0)) {
            for dy in 16..20 {
                chunk.set_sdf(16, dy, 16, -2.0);
            }
        }

        let initial_y = state.position.y;
        let ejected = depenetrate_player(
            &chunks,
            &mut state.position,
            config.standing_height,
            config.radius,
            config.max_depenetration_per_tick,
        );

        assert!(ejected, "Depenetration must detect and eject player");
        let displacement = (state.position.y - initial_y).abs();
        assert!(
            displacement <= config.max_depenetration_per_tick,
            "Depenetration must not teleport > 1.5 m (was {})",
            displacement
        );
    }

    #[test]
    fn test_mov_003_sprint_crouch_and_jump_buffer() {
        let config = CharacterControllerConfig::default();
        let chunks = create_flat_terrain();
        let mut state = CharacterState::new(Vec3::new(16.0, 16.0, 16.0));
        state.on_ground = true;

        // Crouch test
        let crouch_input = CharacterInput {
            crouch: true,
            ..Default::default()
        };
        step_character(&config, &mut state, &crouch_input, &chunks, 1.0 / 60.0);
        assert!(state.is_crouching);
        assert_eq!(state.current_height, config.crouching_height);

        // Jump buffer test
        let jump_input = CharacterInput {
            jump: true,
            ..Default::default()
        };
        step_character(&config, &mut state, &jump_input, &chunks, 1.0 / 60.0);
        assert!(
            state.velocity.y > 0.0,
            "Jump must grant vertical impulse: {}",
            state.velocity.y
        );
    }

    #[test]
    fn test_mov_001_fuzz_walker_no_stuck_no_tunnel() {
        // 10-minute equivalent or 2,000 tick fuzz test over rolling noise terrain
        let config = CharacterControllerConfig::default();
        let mut chunks = HashMap::new();
        for cx in -1..=1 {
            for cz in -1..=1 {
                let pos = ChunkPos::new(cx, 0, cz);
                chunks.insert(pos, us_worldgen::gen_test_chunk(12345, pos));
            }
        }

        let mut state = CharacterState::new(Vec3::new(16.0, 25.0, 16.0));
        let mut rng = Rng64::new(99999);

        for tick in 0..2000 {
            let rx = (rng.next_u64() % 1000) as f32 / 500.0 - 1.0;
            let rz = (rng.next_u64() % 1000) as f32 / 500.0 - 1.0;
            let jump_chance = (rng.next_u64() % 100) < 5;
            let sprint_chance = (rng.next_u64() % 100) < 40;

            let input = CharacterInput {
                move_direction: Vec2::new(rx, rz),
                yaw: (rng.next_u64() % 628) as f32 / 100.0,
                jump: jump_chance,
                sprint: sprint_chance,
                crouch: false,
            };

            step_character(&config, &mut state, &input, &chunks, 1.0 / 60.0);

            // Invariant: Never stuck > 1 s
            assert!(
                state.stuck_timer <= 1.0,
                "Fuzz walker was stuck for > 1s at tick {}",
                tick
            );

            // Invariant: Never tunnels through terrain (capsule center should not be deeply inside solid)
            let feet_sdf = sdf_at_world(&chunks, state.position + Vec3::Y * 0.1);
            assert!(
                feet_sdf >= -0.3,
                "Player tunneled deeply into terrain (SDF: {}) at tick {}",
                feet_sdf,
                tick
            );
        }
    }

    #[test]
    fn test_mov_001_cave_ceiling_and_wall_collision() {
        let config = CharacterControllerConfig::default();
        let pos = ChunkPos::new(0, 0, 0);
        let mut chunk = TerrainChunk::new_air(pos);

        // Build a cave tunnel: floor at y = 16, ceiling at y = 18.5 (2.5m tunnel), wall at x = 12
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let y_f = y as f32;
                    let x_f = x as f32;
                    // Floor is solid below y=16
                    // Ceiling is solid above y=18.5
                    // Wall is solid below x=12
                    let d_floor = y_f - 16.0;
                    let d_ceiling = 18.5 - y_f;
                    let d_wall = x_f - 12.0;
                    let d_tunnel = d_floor.min(d_ceiling).min(d_wall);
                    chunk.sdf.set(x, y, z, us_voxel::quantize_sdf(d_tunnel));
                }
            }
        }

        let mut chunks = HashMap::new();
        chunks.insert(pos, chunk);

        // Player standing on floor at x=16, y=16, z=16
        let mut state = CharacterState::new(Vec3::new(16.0, 16.0, 16.0));
        state.on_ground = true;

        // 1. Jump test against low 2.5m ceiling
        let jump_input = CharacterInput {
            jump: true,
            ..Default::default()
        };
        for _ in 0..20 {
            step_character(&config, &mut state, &jump_input, &chunks, 1.0 / 60.0);
            // Invariant: Top of player head (position.y + standing_height) never punches through ceiling (18.5)
            assert!(
                state.position.y + state.current_height <= 18.55,
                "Player punched through cave ceiling: head at {}",
                state.position.y + state.current_height
            );
        }

        // 2. Walk towards x=12 side wall
        let mut wall_state = CharacterState::new(Vec3::new(14.0, 16.0, 16.0));
        wall_state.on_ground = true;
        let walk_wall_input = CharacterInput {
            move_direction: Vec2::new(-1.0, 0.0), // Walk left toward x=12 wall
            yaw: 0.0,
            ..Default::default()
        };
        for _ in 0..30 {
            step_character(
                &config,
                &mut wall_state,
                &walk_wall_input,
                &chunks,
                1.0 / 60.0,
            );
            // Invariant: Player's left boundary (x - radius) must stop at wall (x >= 12.0)
            assert!(
                wall_state.position.x - config.radius >= 11.95,
                "Player tunneled through cave wall: position.x = {}",
                wall_state.position.x
            );
            // Invariant: Player must NOT step up onto a vertical wall
            assert!(
                wall_state.position.y < 16.5,
                "Player incorrectly stepped up onto cave wall: y = {}",
                wall_state.position.y
            );
        }
    }

    #[test]
    fn test_mov_001_walkable_hill_vs_steep_slope_sliding() {
        let config = CharacterControllerConfig::default();

        // 1. Walkable hill: 55° slope (cos(55°) ≈ 0.574 >= max_slope_cos 0.4226)
        // Surface plane through (16, 16, 16) with normal pointing towards -X and +Y
        let theta_walkable = 55.0f32.to_radians();
        let sin_w = theta_walkable.sin();
        let cos_w = theta_walkable.cos();

        let pos = ChunkPos::new(0, 0, 0);
        let mut chunk_walkable = TerrainChunk::new_air(pos);
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    // SDF: positive in air, negative in solid ground (normal points -X, +Y)
                    let sdf = (y as f32 - 16.0) * cos_w - (x as f32 - 16.0) * sin_w;
                    chunk_walkable.sdf.set(x, y, z, us_voxel::quantize_sdf(sdf));
                }
            }
        }
        let mut chunks_walkable = HashMap::new();
        chunks_walkable.insert(pos, chunk_walkable);

        // Player standing on walkable 55° slope at x=16, y=16.2
        let mut state = CharacterState::new(Vec3::new(16.0, 16.2, 16.0));
        let idle_input = CharacterInput::default();

        // Step ticks to settle onto the hill
        for _ in 0..30 {
            step_character(
                &config,
                &mut state,
                &idle_input,
                &chunks_walkable,
                1.0 / 60.0,
            );
        }

        // On a 55° hill, the player must be grounded and not sliding away
        assert!(
            state.on_ground,
            "Player must be grounded on a 55° hill (max slope is 65°)"
        );
        assert!(
            (state.velocity.x).abs() < 0.1 && (state.velocity.z).abs() < 0.1,
            "Player must not slide on a 55° hill: vel = {:?}",
            state.velocity
        );

        // 2. Very steep hill: 75° slope (cos(75°) ≈ 0.259 < max_slope_cos 0.4226)
        let theta_steep = 75.0f32.to_radians();
        let sin_s = theta_steep.sin();
        let cos_s = theta_steep.cos();

        let mut chunk_steep = TerrainChunk::new_air(pos);
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let sdf = (y as f32 - 16.0) * cos_s - (x as f32 - 16.0) * sin_s;
                    chunk_steep.sdf.set(x, y, z, us_voxel::quantize_sdf(sdf));
                }
            }
        }
        let mut chunks_steep = HashMap::new();
        chunks_steep.insert(pos, chunk_steep);

        // Player placed on steep 75° slope at x=16, y=16.2
        let mut steep_state = CharacterState::new(Vec3::new(16.0, 16.2, 16.0));

        let initial_x = steep_state.position.x;
        let initial_y = steep_state.position.y;

        for _ in 0..30 {
            step_character(
                &config,
                &mut steep_state,
                &idle_input,
                &chunks_steep,
                1.0 / 60.0,
            );
        }

        // On a 75° steep hill, the player must NOT be grounded, and must slide downhill (-X, -Y)
        assert!(
            !steep_state.on_ground,
            "Player must not be grounded on a 75° steep hill"
        );
        assert!(
            steep_state.position.x < initial_x,
            "Player must slide downhill (-X) on 75° steep slope: pos.x = {}, initial = {}",
            steep_state.position.x,
            initial_x
        );
        assert!(
            steep_state.position.y < initial_y,
            "Player must slide downward (-Y) on 75° steep slope: pos.y = {}, initial = {}",
            steep_state.position.y,
            initial_y
        );
    }
}

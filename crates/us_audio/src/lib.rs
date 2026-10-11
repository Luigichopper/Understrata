//! `us_audio`: Audio mixer, material-driven sound sets (AUD-001, AUD-002, §18).
//! Dispatches material-specific audio events for footsteps, digging, and placing.

use bevy::prelude::*;
use us_core::Rng64;
use us_game::PlayerPhysicalState;

/// Material sound profile definition (§18.3, AUD-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialSoundKind {
    Dirt,
    Grass,
    Stone,
    Sand,
    Wood,
    Default,
}

impl MaterialSoundKind {
    pub fn from_material_id(id: u8) -> Self {
        match id {
            1 => MaterialSoundKind::Stone,
            2 => MaterialSoundKind::Dirt,
            3 => MaterialSoundKind::Grass,
            4 => MaterialSoundKind::Sand,
            255 => MaterialSoundKind::Stone, // Bedrock
            _ => MaterialSoundKind::Default,
        }
    }

    pub fn base_pitch(&self) -> f32 {
        match self {
            MaterialSoundKind::Grass => 1.15,
            MaterialSoundKind::Dirt => 0.95,
            MaterialSoundKind::Stone => 0.85,
            MaterialSoundKind::Sand => 1.25,
            MaterialSoundKind::Wood => 1.05,
            MaterialSoundKind::Default => 1.0,
        }
    }
}

/// Event triggered when a player footstep occurs (AUD-002).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct FootstepEvent {
    pub material: MaterialSoundKind,
    pub position: Vec3,
    pub volume: f32,
    pub pitch: f32,
}

/// Event triggered when digging/hitting a material (AUD-002).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct DigSoundEvent {
    pub material: MaterialSoundKind,
    pub position: Vec3,
    pub volume: f32,
    pub pitch: f32,
}

/// Event triggered when placing a block or terrain (AUD-002).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct PlaceSoundEvent {
    pub material: MaterialSoundKind,
    pub position: Vec3,
    pub volume: f32,
    pub pitch: f32,
}

/// Resource tracking accumulated footstep distance based on player movement.
#[derive(Resource)]
pub struct FootstepTracker {
    pub accumulated_distance: f32,
    pub walk_interval: f32,
    pub sprint_interval: f32,
    pub last_position: Vec3,
    pub ground_material: u8,
    pub total_steps_taken: u64,
    rng: Rng64,
}

impl Default for FootstepTracker {
    fn default() -> Self {
        Self {
            accumulated_distance: 0.0,
            walk_interval: 1.8,
            sprint_interval: 2.2,
            last_position: Vec3::ZERO,
            ground_material: 3, // Default to Grass
            total_steps_taken: 0,
            rng: Rng64::new(777111),
        }
    }
}

/// Understrata Audio Plugin
pub struct UnderstrataAudioPlugin;

impl Plugin for UnderstrataAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FootstepTracker>()
            .add_message::<FootstepEvent>()
            .add_message::<DigSoundEvent>()
            .add_message::<PlaceSoundEvent>()
            .add_systems(Update, footstep_system);
    }
}

/// Evaluates player ground movement and generates footstep audio events (AUD-002).
pub fn footstep_system(
    physical_state: Res<PlayerPhysicalState>,
    mut tracker: ResMut<FootstepTracker>,
    mut footstep_events: MessageWriter<FootstepEvent>,
) {
    if !physical_state.on_ground {
        tracker.last_position = physical_state.position;
        return;
    }

    let delta_pos = physical_state.position - tracker.last_position;
    tracker.last_position = physical_state.position;

    let h_dist = Vec2::new(delta_pos.x, delta_pos.z).length();
    if h_dist <= 1e-4 {
        return;
    }

    tracker.accumulated_distance += h_dist;

    let is_sprinting = physical_state.velocity.length_squared() > (5.0 * 5.0);
    let interval = if is_sprinting {
        tracker.sprint_interval
    } else {
        tracker.walk_interval
    };

    if tracker.accumulated_distance >= interval {
        tracker.accumulated_distance = 0.0;
        tracker.total_steps_taken += 1;

        let mat_kind = MaterialSoundKind::from_material_id(tracker.ground_material);
        let pitch_jitter = ((tracker.rng.next_u64() % 100) as f32 / 500.0) - 0.1; // -0.1 .. +0.1
        let volume = if is_sprinting { 0.85 } else { 0.65 };

        footstep_events.write(FootstepEvent {
            material: mat_kind,
            position: physical_state.position,
            volume,
            pitch: (mat_kind.base_pitch() + pitch_jitter).max(0.2),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aud_002_material_sound_profiles() {
        assert_eq!(
            MaterialSoundKind::from_material_id(1),
            MaterialSoundKind::Stone
        );
        assert_eq!(
            MaterialSoundKind::from_material_id(2),
            MaterialSoundKind::Dirt
        );
        assert_eq!(
            MaterialSoundKind::from_material_id(3),
            MaterialSoundKind::Grass
        );

        assert!(MaterialSoundKind::Stone.base_pitch() < MaterialSoundKind::Grass.base_pitch());
    }

    #[test]
    fn test_aud_002_footstep_accumulation_and_event() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(UnderstrataAudioPlugin)
            .init_resource::<PlayerPhysicalState>();

        // Set player on ground and moving
        {
            let mut state = app.world_mut().resource_mut::<PlayerPhysicalState>();
            state.on_ground = true;
            state.position = Vec3::new(0.0, 16.0, 0.0);
        }

        app.update();

        // Move player forward by 2.0 meters (clearing 1.8m step interval)
        {
            let mut state = app.world_mut().resource_mut::<PlayerPhysicalState>();
            state.position = Vec3::new(0.0, 16.0, 2.0);
        }

        app.update();

        let tracker = app.world().resource::<FootstepTracker>();
        assert_eq!(
            tracker.total_steps_taken, 1,
            "Walking > 1.8m must trigger 1 footstep"
        );

        let messages = app.world().resource::<Messages<FootstepEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let collected: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].material, MaterialSoundKind::Grass);
    }
}

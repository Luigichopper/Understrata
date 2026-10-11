//! `us_game`: Gameplay logic, ECS systems, character controller integration, and authoritative server loop (§4.1, §17).

pub mod player;
pub mod server;

use bevy::prelude::*;
pub use player::{
    player_input_system, player_reconciliation_system, LocalPlayer, PlayerInputState,
    PlayerPhysicalState,
};
pub use server::{local_server_tick_system, LocalServer};

/// Core Understrata Game Plugin (M3: Walk the World, local server, player movement, intent architecture).
pub struct UnderstrataGamePlugin;

impl Plugin for UnderstrataGamePlugin {
    fn build(&self, app: &mut App) {
        let spawn_pos = glam::Vec3::new(0.0, 24.0, 42.0);
        app.insert_resource(LocalServer::new(0, 0.0, spawn_pos))
            .init_resource::<PlayerInputState>()
            .init_resource::<PlayerPhysicalState>()
            .add_systems(
                Update,
                (
                    player_input_system,
                    local_server_tick_system,
                    player_reconciliation_system,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{IVec3, Vec2, Vec3};
    use us_core::ChunkPos;
    use us_net::{ClientMessage, EditIntent, PlayerInputIntent, ServerMessage};
    use us_voxel::{BrushMode, BrushShape, EditOp, TerrainChunk};

    #[test]
    fn test_net_001_sp_mutations_only_via_server_systems() {
        // Acceptance criteria for NET-001:
        // "SP gameplay never mutates world outside server systems (lint/test enforced)"
        let spawn_pos = Vec3::new(16.0, 20.0, 16.0);
        let mut server = LocalServer::new(0, 0.0, spawn_pos);

        // Prepopulate server with a solid chunk
        let cpos = ChunkPos::new(0, 0, 0);
        server.chunks.insert(cpos, TerrainChunk::new_solid(cpos, 1));

        assert_eq!(server.op_log.len(), 0);
        assert_eq!(server.server_op_seq, 0);

        // 1. Client creates an edit intent (requesting to dig a hole)
        let edit_op = EditOp::TerrainBrush {
            center_q: IVec3::new(16 * 16, 16 * 16, 16 * 16),
            radius_q: 32,
            shape: BrushShape::Sphere,
            mode: BrushMode::Subtract,
            material: 0,
            strength: 255,
        };

        let client_intent = ClientMessage::EditIntent(EditIntent {
            sequence: 1,
            op: edit_op.clone(),
        });

        // 2. Submit intent through client transport
        server.send_from_client(client_intent, 0.0);

        // Prior to server ticking, server world has NOT mutated
        assert_eq!(server.op_log.len(), 0);

        // 3. Server system ticks and processes intents
        server.tick_server(0.0, 1.0 / 60.0);

        // 4. Invariant: Authoritative server state mutated and logged
        assert_eq!(server.op_log.len(), 1, "Server must log authoritative op");
        assert_eq!(server.server_op_seq, 1);
        assert_eq!(server.op_log[0].op, edit_op);

        // 5. Invariant: Server emitted authoritative EditOpDelta to client
        let client_inbox = server.poll_client_inbox(0.0);
        assert_eq!(client_inbox.len(), 1);
        match &client_inbox[0] {
            ServerMessage::EditOpDelta(delta) => {
                assert_eq!(delta.op_sequence, 1);
                assert_eq!(delta.op, edit_op);
            }
            other => panic!("Expected EditOpDelta, got {:?}", other),
        }

        // 6. Test movement intent flows exclusively through server physics
        let move_intent = ClientMessage::PlayerInput(PlayerInputIntent {
            sequence: 10,
            move_dir: Vec2::new(0.0, 1.0),
            look_yaw: 0.0,
            look_pitch: 0.0,
            jump: false,
            sprint: false,
            crouch: false,
            dt: 1.0 / 60.0,
        });

        server.send_from_client(move_intent, 0.1);
        server.tick_server(0.1, 1.0 / 60.0);

        let client_inbox = server.poll_client_inbox(0.1);
        assert_eq!(client_inbox.len(), 1);
        match &client_inbox[0] {
            ServerMessage::PlayerState(state_delta) => {
                assert_eq!(state_delta.sequence_ack, 10);
                assert_eq!(state_delta.position, server.player_state.position);
            }
            other => panic!("Expected PlayerState delta, got {:?}", other),
        }
    }
}

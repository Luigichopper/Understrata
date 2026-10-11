//! Authoritative In-Process Server for Single Player & Co-op (§4.1, §17.1, NET-001).
//! Enforces: ALL world mutations and physics authority execute on server systems.

use bevy::prelude::*;
use std::collections::HashMap;
use us_core::ChunkPos;
use us_net::{
    ClientMessage, EditIntent, EditOpDelta, NetworkSimulator, PlayerInputIntent, PlayerStateDelta,
    ServerMessage,
};
use us_sim::{
    depenetrate_player, step_character, CharacterControllerConfig, CharacterInput, CharacterState,
};
use us_voxel::{apply_edit_op, LoggedOp, TerrainChunk};

/// Authoritative server resource managing simulated network queues and world state (NET-001, NET-010).
#[derive(Resource)]
pub struct LocalServer {
    /// Authoritative server copy of world chunks.
    pub chunks: HashMap<ChunkPos, TerrainChunk>,
    /// Log of all authoritative edit operations applied to the world (TER-009).
    pub op_log: Vec<LoggedOp>,
    /// Sequence counter for server operations.
    pub server_op_seq: u64,
    /// Authoritative state of the connected local player.
    pub player_state: CharacterState,
    pub player_config: CharacterControllerConfig,
    pub last_input_sequence: u64,
    /// Client-to-server simulated transport (applies lag and loss).
    pub client_to_server: NetworkSimulator<ClientMessage>,
    /// Server-to-client simulated transport (applies lag and loss).
    pub server_to_client: NetworkSimulator<ServerMessage>,
}

impl LocalServer {
    pub fn new(lag_ms: u32, loss_pct: f32, spawn_pos: glam::Vec3) -> Self {
        Self {
            chunks: HashMap::new(),
            op_log: Vec::new(),
            server_op_seq: 0,
            player_state: CharacterState::new(spawn_pos),
            player_config: CharacterControllerConfig::default(),
            last_input_sequence: 0,
            client_to_server: NetworkSimulator::new(lag_ms, loss_pct, 1001),
            server_to_client: NetworkSimulator::new(lag_ms, loss_pct, 1002),
        }
    }

    /// Submits a client message into the transport pipe heading to the server.
    pub fn send_from_client(&mut self, msg: ClientMessage, time_secs: f64) -> bool {
        self.client_to_server.send(msg, time_secs)
    }

    /// Polls server messages delivered to the client.
    pub fn poll_client_inbox(&mut self, time_secs: f64) -> Vec<ServerMessage> {
        self.server_to_client.receive(time_secs)
    }

    /// Authoritative server tick: processes queued client intents, runs physics, and emits deltas.
    pub fn tick_server(&mut self, time_secs: f64, dt: f32) {
        let incoming = self.client_to_server.receive(time_secs);

        for msg in incoming {
            match msg {
                ClientMessage::Connect {
                    client_version,
                    player_name,
                } => {
                    let response = us_net::validate_client_connection(client_version, &player_name);
                    self.server_to_client.send(response, time_secs);
                }
                ClientMessage::PlayerInput(intent) => {
                    self.handle_player_input(intent, dt, time_secs);
                }
                ClientMessage::EditIntent(intent) => {
                    self.handle_edit_intent(intent, time_secs);
                }
                ClientMessage::Ping { nonce, timestamp } => {
                    self.server_to_client
                        .send(ServerMessage::Pong { nonce, timestamp }, time_secs);
                }
                ClientMessage::Disconnect => {
                    info!("Client disconnected from local server.");
                }
            }
        }
    }

    fn handle_player_input(&mut self, intent: PlayerInputIntent, dt: f32, time_secs: f64) {
        if intent.sequence <= self.last_input_sequence {
            return; // Ignore out-of-order or duplicate inputs
        }
        self.last_input_sequence = intent.sequence;

        let input = CharacterInput {
            move_direction: intent.move_dir,
            yaw: intent.look_yaw,
            jump: intent.jump,
            sprint: intent.sprint,
            crouch: intent.crouch,
        };

        // Execute kinematic character physics on authoritative server
        step_character(
            &self.player_config,
            &mut self.player_state,
            &input,
            &self.chunks,
            dt,
        );

        // Emit authoritative state delta
        let delta = ServerMessage::PlayerState(PlayerStateDelta {
            sequence_ack: intent.sequence,
            position: self.player_state.position,
            velocity: self.player_state.velocity,
            on_ground: self.player_state.on_ground,
            is_crouching: self.player_state.is_crouching,
        });

        self.server_to_client.send(delta, time_secs);
    }

    fn handle_edit_intent(&mut self, intent: EditIntent, time_secs: f64) {
        self.server_op_seq += 1;
        let op_seq = self.server_op_seq;

        // Authoritatively mutate server chunks
        let _applied = apply_edit_op(&mut self.chunks, &intent.op);

        // Record authoritative op log (TER-009)
        self.op_log.push(LoggedOp {
            seq: op_seq,
            tick: 0,
            actor: 0,
            op: intent.op.clone(),
        });

        // MOV-002: Post-edit depenetration for any players intersecting modified terrain
        depenetrate_player(
            &self.chunks,
            &mut self.player_state.position,
            self.player_state.current_height,
            self.player_config.radius,
            self.player_config.max_depenetration_per_tick,
        );

        // Replicate edit delta to clients (NET-001)
        let edit_delta = ServerMessage::EditOpDelta(EditOpDelta {
            op_sequence: op_seq,
            op: intent.op,
        });
        self.server_to_client.send(edit_delta, time_secs);
    }
}

/// System running the authoritative local server tick each frame.
pub fn local_server_tick_system(time: Res<Time>, mut server: ResMut<LocalServer>) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    server.tick_server(now, dt);
}

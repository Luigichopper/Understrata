//! `us_net`: Network protocol messages, edit op serialization, and network simulation (§4.1, §17).
//! Pure crate: NO Bevy dependencies.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use us_voxel::EditOp;

/// Current network protocol version (NET-002).
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Protocol version mismatch: expected {expected}, received {received}")]
    VersionMismatch { expected: u32, received: u32 },
    #[error("Payload decoding error: {0}")]
    DecodeError(String),
    #[error("Payload encoding error: {0}")]
    EncodeError(String),
    #[error("Packet too short (minimum {0} bytes required)")]
    PacketTooShort(usize),
}

/// Client input intent sent to the authoritative server (NET-001, §7.1, §17.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInputIntent {
    pub sequence: u64,
    pub move_dir: Vec2,
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub jump: bool,
    pub sprint: bool,
    pub crouch: bool,
    pub dt: f32,
}

/// Client edit request sent to the authoritative server (NET-001, §5.6, §17.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditIntent {
    pub sequence: u64,
    pub op: EditOp,
}

/// Authoritative player state delta replicated from server to client (§17.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerStateDelta {
    pub sequence_ack: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_crouching: bool,
}

/// Authoritative terrain edit op replicated from server to client (§5.6, §17.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditOpDelta {
    pub op_sequence: u64,
    pub op: EditOp,
}

/// Top-level client message envelope (NET-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Connect {
        client_version: u32,
        player_name: String,
    },
    PlayerInput(PlayerInputIntent),
    EditIntent(EditIntent),
    Ping {
        nonce: u64,
        timestamp: f64,
    },
    Disconnect,
}

/// Top-level server message envelope (NET-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        server_version: u32,
        tick_rate: u32,
        player_id: u32,
    },
    RejectVersion {
        server_version: u32,
        client_version: u32,
        reason: String,
    },
    PlayerState(PlayerStateDelta),
    EditOpDelta(EditOpDelta),
    Pong {
        nonce: u64,
        timestamp: f64,
    },
}

/// Encodes a client message into version-prefixed bytes.
pub fn encode_client_message(msg: &ClientMessage) -> Result<Vec<u8>, ProtocolError> {
    let payload = ron::to_string(msg).map_err(|e| ProtocolError::EncodeError(e.to_string()))?;
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    bytes.extend_from_slice(payload.as_bytes());
    Ok(bytes)
}

/// Decodes a client message from version-prefixed bytes.
pub fn decode_client_message(bytes: &[u8]) -> Result<ClientMessage, ProtocolError> {
    if bytes.len() < 4 {
        return Err(ProtocolError::PacketTooShort(4));
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::VersionMismatch {
            expected: PROTOCOL_VERSION,
            received: version,
        });
    }
    let payload_str =
        std::str::from_utf8(&bytes[4..]).map_err(|e| ProtocolError::DecodeError(e.to_string()))?;
    ron::from_str(payload_str).map_err(|e| ProtocolError::DecodeError(e.to_string()))
}

/// Encodes a server message into version-prefixed bytes.
pub fn encode_server_message(msg: &ServerMessage) -> Result<Vec<u8>, ProtocolError> {
    let payload = ron::to_string(msg).map_err(|e| ProtocolError::EncodeError(e.to_string()))?;
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    bytes.extend_from_slice(payload.as_bytes());
    Ok(bytes)
}

/// Decodes a server message from version-prefixed bytes.
pub fn decode_server_message(bytes: &[u8]) -> Result<ServerMessage, ProtocolError> {
    if bytes.len() < 4 {
        return Err(ProtocolError::PacketTooShort(4));
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::VersionMismatch {
            expected: PROTOCOL_VERSION,
            received: version,
        });
    }
    let payload_str =
        std::str::from_utf8(&bytes[4..]).map_err(|e| ProtocolError::DecodeError(e.to_string()))?;
    ron::from_str(payload_str).map_err(|e| ProtocolError::DecodeError(e.to_string()))
}

/// Validates client version on connection handshake.
pub fn validate_client_connection(client_version: u32, player_name: &str) -> ServerMessage {
    if client_version != PROTOCOL_VERSION {
        ServerMessage::RejectVersion {
            server_version: PROTOCOL_VERSION,
            client_version,
            reason: format!(
                "Incompatible client protocol version {}. Server is on version {}.",
                client_version, PROTOCOL_VERSION
            ),
        }
    } else {
        ServerMessage::Welcome {
            server_version: PROTOCOL_VERSION,
            tick_rate: 60,
            player_id: us_core::hash_coords3(0, player_name.len() as i32, 0, 0) as u32,
        }
    }
}

/// Network latency and packet loss simulator for local development / testing (NET-010).
#[derive(Debug, Clone)]
pub struct NetworkSimulator<T> {
    pub lag_ms: u32,
    pub loss_pct: f32,
    rng_state: u64,
    queue: Vec<(f64, T)>,
}

impl<T> NetworkSimulator<T> {
    pub fn new(lag_ms: u32, loss_pct: f32, seed: u64) -> Self {
        Self {
            lag_ms,
            loss_pct: loss_pct.clamp(0.0, 100.0),
            rng_state: if seed == 0 { 0xdeadbeef } else { seed },
            queue: Vec::new(),
        }
    }

    /// Linear congruential generator for fast deterministic packet loss simulation.
    fn next_rand_uniform(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        let val = (self.rng_state >> 32) as u32;
        (val as f32) / (u32::MAX as f32)
    }

    /// Enqueues a packet. If simulated packet loss triggers, the packet is dropped immediately.
    pub fn send(&mut self, item: T, current_time_secs: f64) -> bool {
        if self.loss_pct > 0.0 {
            let roll = self.next_rand_uniform() * 100.0;
            if roll < self.loss_pct {
                return false; // Dropped packet!
            }
        }

        let deliver_at = current_time_secs + (self.lag_ms as f64 / 1000.0);
        self.queue.push((deliver_at, item));
        true
    }

    /// Receives all packets whose delivery time has arrived.
    pub fn receive(&mut self, current_time_secs: f64) -> Vec<T> {
        let mut ready = Vec::new();
        let mut remaining = Vec::with_capacity(self.queue.len());

        for (deliver_at, item) in self.queue.drain(..) {
            if deliver_at <= current_time_secs {
                ready.push(item);
            } else {
                remaining.push((deliver_at, item));
            }
        }

        self.queue = remaining;
        ready
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;
    use us_voxel::{BrushMode, BrushShape};

    #[test]
    fn test_net_002_client_message_roundtrip() {
        let input_msg = ClientMessage::PlayerInput(PlayerInputIntent {
            sequence: 42,
            move_dir: Vec2::new(1.0, -0.5),
            look_yaw: 1.57,
            look_pitch: -0.2,
            jump: true,
            sprint: false,
            crouch: false,
            dt: 1.0 / 60.0,
        });

        let encoded = encode_client_message(&input_msg).expect("Failed to encode");
        let decoded = decode_client_message(&encoded).expect("Failed to decode");
        assert_eq!(input_msg, decoded);

        let edit_msg = ClientMessage::EditIntent(EditIntent {
            sequence: 43,
            op: EditOp::TerrainBrush {
                center_q: IVec3::new(16, 32, 48),
                radius_q: 32,
                shape: BrushShape::Sphere,
                mode: BrushMode::Subtract,
                material: 1,
                strength: 255,
            },
        });

        let encoded_edit = encode_client_message(&edit_msg).expect("Failed to encode");
        let decoded_edit = decode_client_message(&encoded_edit).expect("Failed to decode");
        assert_eq!(edit_msg, decoded_edit);
    }

    #[test]
    fn test_net_002_server_message_roundtrip() {
        let state_msg = ServerMessage::PlayerState(PlayerStateDelta {
            sequence_ack: 101,
            position: Vec3::new(12.5, 34.0, -5.2),
            velocity: Vec3::new(0.0, -9.8, 1.2),
            on_ground: true,
            is_crouching: false,
        });

        let encoded = encode_server_message(&state_msg).expect("Failed to encode");
        let decoded = decode_server_message(&encoded).expect("Failed to decode");
        assert_eq!(state_msg, decoded);
    }

    #[test]
    fn test_net_002_version_mismatch_rejected() {
        // Form a raw packet with an invalid protocol version (version 999)
        let payload = ron::to_string(&ClientMessage::Disconnect).unwrap();
        let mut bad_packet = Vec::new();
        bad_packet.extend_from_slice(&999u32.to_le_bytes());
        bad_packet.extend_from_slice(payload.as_bytes());

        let res = decode_client_message(&bad_packet);
        match res {
            Err(ProtocolError::VersionMismatch { expected, received }) => {
                assert_eq!(expected, PROTOCOL_VERSION);
                assert_eq!(received, 999);
            }
            other => panic!("Expected VersionMismatch, got {:?}", other),
        }

        // Test handshake validator
        let rejection = validate_client_connection(999, "Alice");
        match rejection {
            ServerMessage::RejectVersion {
                server_version,
                client_version,
                reason,
            } => {
                assert_eq!(server_version, PROTOCOL_VERSION);
                assert_eq!(client_version, 999);
                assert!(reason.contains("Incompatible client protocol version"));
            }
            _ => panic!("Expected RejectVersion"),
        }
    }

    #[test]
    fn test_net_010_network_simulator_lag_and_loss() {
        // Test lag
        let mut sim = NetworkSimulator::new(100, 0.0, 12345); // 100 ms lag, 0% loss
        let sent = sim.send("hello", 1.0);
        assert!(sent);

        // At 1.05s (50ms elapsed), message should not be delivered yet
        let received = sim.receive(1.05);
        assert!(received.is_empty());
        assert_eq!(sim.pending_count(), 1);

        // At 1.10s (100ms elapsed), message should be delivered
        let received = sim.receive(1.10);
        assert_eq!(received, vec!["hello"]);
        assert_eq!(sim.pending_count(), 0);

        // Test loss (100% loss)
        let mut dropping_sim = NetworkSimulator::new(0, 100.0, 12345);
        let sent = dropping_sim.send("drop_me", 1.0);
        assert!(!sent);
        assert_eq!(dropping_sim.pending_count(), 0);
    }
}

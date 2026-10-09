//! Exact per-tick comparisons. Floating-point state is stored as u32 bit patterns.
//! A passing pair only covers its recorded fields/scenario; it is not a fidelity claim.
use crate::{
    import::version,
    simulation::{TICKS_PER_SECOND, TickInput},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

pub const TRACE_SCHEMA: u32 = 1;
pub const INPUT_LOG_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectState {
    pub id: u32,
    pub behavior: u16,
    pub action: u32,
    pub timer: u32,
    pub position_bits: [u32; 3],
    pub velocity_bits: [u32; 3],
    pub fields: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickState {
    pub action: u32,
    pub action_state: u16,
    pub action_timer: u16,
    pub position_bits: [u32; 3],
    pub velocity_bits: [u32; 3],
    pub face_angles: [i16; 3],
    pub camera_yaw: i16,
    pub rng: u16,
    /// Stable surface IDs assigned from preserved source order, not ROM pointers.
    pub contacts: Vec<u32>,
    pub interactions: Vec<u32>,
    /// Original update order, not sorted by ID before comparison.
    pub objects: Vec<ObjectState>,
    /// Add fields as action families require them; both producers must agree.
    pub fields: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub rom_sha1: String,
    pub reference_revision: String,
    pub reference_configuration: String,
    pub scenario: String,
    pub tick_rate: u32,
    pub course: u8,
    pub area: u8,
    pub act: u8,
    pub initial_world_digest: String,
    pub gameplay_options: BTreeMap<String, String>,
    pub initial_state: TickState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub tick: u64,
    pub input: TickInput,
    pub state: TickState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub schema: u32,
    /// May differ between oracle and Rust; this is not gameplay configuration.
    pub producer: String,
    pub metadata: Metadata,
    pub frames: Vec<Frame>,
}

/// One play session's tick inputs from a named level entry, for replaying
/// the session against the reference. The inputs are the player's, not ROM
/// data; the ROM identity is kept so a replay can check it uses the same
/// content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputLog {
    pub schema: u32,
    pub producer: String,
    pub rom_sha1: String,
    /// The level entry the first input follows, such as `bob-script-start`.
    pub entry: String,
    pub tick_rate: u32,
    pub inputs: Vec<TickInput>,
}

impl InputLog {
    /// Parse a log and check its schema and tick rate.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let log: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if log.schema != INPUT_LOG_SCHEMA {
            return Err(format!("unsupported input log schema {}", log.schema));
        }
        if log.tick_rate != TICKS_PER_SECOND {
            return Err(format!(
                "input log tick rate {} is not {TICKS_PER_SECOND}",
                log.tick_rate
            ));
        }
        Ok(log)
    }

    pub fn to_json_pretty(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("integer input log serialization")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub tick: Option<u64>,
    pub field: String,
    pub expected: String,
    pub actual: String,
}
impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(tick) = self.tick {
            write!(f, "tick {tick}: ")?;
        }
        write!(
            f,
            "{}: expected {}, got {}",
            self.field, self.expected, self.actual
        )
    }
}
impl std::error::Error for Divergence {}

fn validate(trace: &Trace) -> Result<(), String> {
    if trace.schema != TRACE_SCHEMA {
        return Err("unsupported trace schema".into());
    }
    if trace.metadata.tick_rate != TICKS_PER_SECOND {
        return Err("expected 30 ticks/second".into());
    }
    if trace.metadata.rom_sha1 != version::US_SHA1 && trace.metadata.rom_sha1 != "synthetic" {
        return Err("unsupported ROM identity in trace".into());
    }
    if trace.metadata.reference_revision != version::REFERENCE_REVISION {
        return Err("trace reference does not match pinned oracle revision".into());
    }
    if !(1..=6).contains(&trace.metadata.act) || trace.metadata.area == 0 {
        return Err("invalid act or area".into());
    }
    if trace.frames.is_empty() {
        return Err("empty replay is not comparison coverage".into());
    }
    if trace.producer.is_empty()
        || trace.metadata.reference_configuration.is_empty()
        || trace.metadata.scenario.is_empty()
        || trace.metadata.initial_world_digest.is_empty()
    {
        return Err("missing scenario, producer, configuration, or world identity".into());
    }
    for (i, frame) in trace.frames.iter().enumerate() {
        if frame.tick != i as u64 + 1 {
            return Err(format!("tick sequence must start at 1; frame {i}"));
        }
        let mut ids = std::collections::BTreeSet::new();
        if frame.state.objects.iter().any(|o| !ids.insert(o.id)) {
            return Err(format!("duplicate stable object ID at tick {}", frame.tick));
        }
    }
    Ok(())
}

/// Recursive, stable field order; authoritative numbers are integer bits.
fn first_difference(
    expected: &Value,
    actual: &Value,
    path: &str,
) -> Option<(String, String, String)> {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                let next = format!("{path}.{key}");
                let Some(other) = b.get(key) else {
                    return Some((next, value.to_string(), "missing".into()));
                };
                if let Some(d) = first_difference(value, other, &next) {
                    return Some(d);
                }
            }
            for (key, value) in b {
                if !a.contains_key(key) {
                    return Some((format!("{path}.{key}"), "missing".into(), value.to_string()));
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = first_difference(x, y, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            if a.len() != b.len() {
                return Some((
                    format!("{path}.length"),
                    a.len().to_string(),
                    b.len().to_string(),
                ));
            }
        }
        _ if expected != actual => {
            return Some((path.into(), expected.to_string(), actual.to_string()));
        }
        _ => {}
    }
    None
}

pub fn compare(expected: &Trace, actual: &Trace) -> Result<(), Divergence> {
    for (label, trace) in [("reference", expected), ("candidate", actual)] {
        validate(trace).map_err(|message| Divergence {
            tick: None,
            field: format!("{label}.validation"),
            expected: "valid nonempty trace".into(),
            actual: message,
        })?;
    }
    let a = serde_json::to_value(&expected.metadata).expect("integer trace serialization");
    let b = serde_json::to_value(&actual.metadata).expect("integer trace serialization");
    if let Some((field, expected, actual)) = first_difference(&a, &b, "metadata") {
        return Err(Divergence {
            tick: None,
            field,
            expected,
            actual,
        });
    }
    for (a, b) in expected.frames.iter().zip(&actual.frames) {
        let x = serde_json::to_value(a).expect("integer trace serialization");
        let y = serde_json::to_value(b).expect("integer trace serialization");
        if let Some((field, expected, actual)) = first_difference(&x, &y, "frame") {
            return Err(Divergence {
                tick: Some(a.tick),
                field,
                expected,
                actual,
            });
        }
    }
    if expected.frames.len() != actual.frames.len() {
        return Err(Divergence {
            tick: Some(expected.frames.len().min(actual.frames.len()) as u64 + 1),
            field: "frames.length".into(),
            expected: expected.frames.len().to_string(),
            actual: actual.frames.len().to_string(),
        });
    }
    Ok(())
}

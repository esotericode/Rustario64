//! Collision stream layouts and force-bearing surface types follow CC0 sm64.
//! This decodes records only; no reference collision-query behavior is claimed.
use super::{ImportError, Result, reader::Reader, special};
use crate::content::{CollisionMesh, SpecialPlacement, Triangle};

pub const MAX_VERTICES: usize = 65535;
pub const MAX_TRIANGLES: usize = 100000;

fn count(r: Reader<'_>, at: usize) -> Result<usize> {
    let value = r.i16(at)?;
    usize::try_from(value).map_err(|_| ImportError::new("collision count", at, "negative count"))
}

pub fn decode(bytes: &[u8]) -> Result<(CollisionMesh, usize)> {
    let r = Reader::new(bytes, "collision");
    if r.u16(0)? != 0x40 {
        return Err(ImportError::new(
            "collision",
            0,
            "expected vertex command 0x40",
        ));
    }
    let mut mesh = CollisionMesh {
        vertices: vec![],
        triangles: vec![],
        specials: vec![],
        environment: vec![],
    };
    let mut at = 0usize;
    let mut vertex_base = 0usize;
    let mut vertex_count = 0usize;
    let mut stopped = false;
    loop {
        let command_at = at;
        let command = r.u16(at)?;
        at += 2;
        match command {
            0x40 => {
                if stopped {
                    return Err(ImportError::new(
                        "collision",
                        command_at,
                        "vertices after triangle stop",
                    ));
                }
                vertex_count = count(r, at)?;
                at += 2;
                vertex_base = mesh.vertices.len();
                if vertex_base + vertex_count > MAX_VERTICES {
                    return Err(ImportError::new("collision", at, "vertex limit exceeded"));
                }
                r.slice(at, vertex_count * 6)?;
                for _ in 0..vertex_count {
                    mesh.vertices
                        .push([r.i16(at)?, r.i16(at + 2)?, r.i16(at + 4)?]);
                    at += 6;
                }
            }
            0x41 => stopped = true,
            0x42 => return Ok((mesh, at)),
            0x43 => {
                let n = count(r, at)?;
                at += 2;
                for _ in 0..n {
                    let raw_id = r.u16(at)?;
                    let preset = u8::try_from(raw_id).map_err(|_| {
                        ImportError::new("collision special", at, "preset does not fit u8")
                    })?;
                    let words = special::extra_words(preset).ok_or_else(|| {
                        ImportError::new(
                            "collision special",
                            at,
                            format!("unsupported preset 0x{preset:02X}"),
                        )
                    })?;
                    let position = [r.i16(at + 2)?, r.i16(at + 4)?, r.i16(at + 6)?];
                    at += 8;
                    let mut extra = Vec::with_capacity(words);
                    for _ in 0..words {
                        extra.push(r.i16(at)?);
                        at += 2;
                    }
                    mesh.specials.push(SpecialPlacement {
                        preset,
                        position,
                        extra,
                    });
                }
            }
            0x44 => {
                // gEnvironmentRegions points at the latest block only; merging
                // several blocks would change query results, so reject them.
                if !mesh.environment.is_empty() {
                    return Err(ImportError::new(
                        "collision",
                        command_at,
                        "multiple environment-region blocks are not supported",
                    ));
                }
                let n = count(r, at)?;
                at += 2;
                r.slice(at, n * 12)?;
                for _ in 0..n {
                    let mut region = [0; 6];
                    for value in &mut region {
                        *value = r.i16(at)?;
                        at += 2;
                    }
                    mesh.environment.push(region);
                }
            }
            surface if surface < 0x40 || (0x65..=0xFF).contains(&surface) => {
                if stopped {
                    return Err(ImportError::new(
                        "collision",
                        command_at,
                        "surface after triangle stop",
                    ));
                }
                let n = count(r, at)?;
                at += 2;
                if mesh.triangles.len() + n > MAX_TRIANGLES {
                    return Err(ImportError::new("collision", at, "triangle limit exceeded"));
                }
                let has_force = matches!(surface, 0x04 | 0x0E | 0x24 | 0x25 | 0x27 | 0x2C | 0x2D);
                r.slice(at, n * if has_force { 8 } else { 6 })?;
                for _ in 0..n {
                    let raw = [r.u16(at)?, r.u16(at + 2)?, r.u16(at + 4)?];
                    if raw.iter().any(|&v| usize::from(v) >= vertex_count) {
                        return Err(ImportError::new(
                            "collision triangle",
                            at,
                            "vertex index out of range",
                        ));
                    }
                    let indices = raw.map(|v| (vertex_base + usize::from(v)) as u16);
                    let force = if has_force {
                        Some(r.i16(at + 6)?)
                    } else {
                        None
                    };
                    mesh.triangles.push(Triangle {
                        indices,
                        surface: surface as i16,
                        force,
                    });
                    at += if has_force { 8 } else { 6 };
                }
            }
            _ => {
                return Err(ImportError::new(
                    "collision",
                    command_at,
                    format!("unsupported command 0x{command:04X}"),
                ));
            }
        }
    }
}

/// Inspectable original collision, deliberately separate from visible terrain.
pub fn to_obj(mesh: &CollisionMesh) -> String {
    use std::fmt::Write;
    let mut out = String::from("# Rustario64 collision inspection; not a visible terrain mesh\n");
    for [x, y, z] in &mesh.vertices {
        writeln!(out, "v {x} {y} {z}").expect("String write");
    }
    let mut last = None;
    for triangle in &mesh.triangles {
        if last != Some(triangle.surface) {
            writeln!(out, "g surface_{:04X}", triangle.surface).expect("String write");
            last = Some(triangle.surface);
        }
        let [a, b, c] = triangle.indices.map(|i| u32::from(i) + 1);
        writeln!(out, "f {a} {b} {c}").expect("String write");
    }
    out
}

//! Minimal OBJ loader returning MeshData for simple triangle meshes
//! KISS: positions + optional normals, faces triangulated; ignores materials/UVs.

use super::gltf_loader::MeshData;
use super::math::Vector3;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[inline]
fn parse_index(tok: &str, len: usize) -> Option<usize> {
    // OBJ indices are 1-based; negative indices are relative to end
    if tok.is_empty() {
        return None;
    }
    if let Ok(mut idx) = tok.parse::<i32>() {
        if idx > 0 {
            return Some((idx as usize) - 1);
        }
        // negative
        let n = len as i32;
        idx = n + idx; // idx is negative
        if idx >= 0 {
            return Some(idx as usize);
        }
    }
    None
}

/// Load a very simple OBJ (triangles/quads) into MeshData
pub fn load_obj<P: AsRef<Path>>(path: P) -> Result<MeshData> {
    let file = File::open(&path)
        .with_context(|| format!("Failed to open OBJ file: {}", path.as_ref().display()))?;
    let reader = BufReader::new(file);

    let mut positions: Vec<Vector3> = Vec::new();
    let mut normals: Vec<Vector3> = Vec::new();
    let mut out_positions: Vec<Vector3> = Vec::new();
    let mut out_normals: Vec<Vector3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Track normals completeness
    let mut has_any_normals: bool = false;
    let mut has_any_missing_normals: bool = false;

    // Map of (v_idx, vn_idx) -> new vertex index
    use std::collections::HashMap;
    let mut vert_map: HashMap<(usize, Option<usize>), u32> = HashMap::new();

    for line_res in reader.lines() {
        let line = line_res?;
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s.starts_with("v ") {
            // position
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 4 {
                let x: f32 = parts[1].parse().unwrap_or(0.0);
                let y: f32 = parts[2].parse().unwrap_or(0.0);
                let z: f32 = parts[3].parse().unwrap_or(0.0);
                positions.push(Vector3::new(x, y, z));
            }
        } else if s.starts_with("vn ") {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 4 {
                let x: f32 = parts[1].parse().unwrap_or(0.0);
                let y: f32 = parts[2].parse().unwrap_or(0.0);
                let z: f32 = parts[3].parse().unwrap_or(0.0);
                normals.push(Vector3::new(x, y, z));
            }
        } else if s.starts_with("f ") {
            // faces: triangulate fan
            let mut face_verts: Vec<u32> = Vec::new();
            for tok in s[2..].split_whitespace() {
                // formats: v, v//vn, v/vt, v/vt/vn
                let mut v_idx: Option<usize> = None;
                let mut vn_idx: Option<usize> = None;
                let mut parts = tok.split('/');
                if let Some(a) = parts.next() {
                    v_idx = parse_index(a, positions.len());
                }
                if let Some(_vt) = parts.next() {
                    if let Some(c) = parts.next() {
                        // v/vt/vn
                        vn_idx = parse_index(c, normals.len());
                    } else {
                        // v/vt
                    }
                }
                // handle v//vn pattern
                if tok.contains("//") {
                    let split: Vec<&str> = tok.split("//").collect();
                    if split.len() == 2 {
                        v_idx = parse_index(split[0], positions.len());
                        vn_idx = parse_index(split[1], normals.len());
                    }
                }
                if let Some(vi) = v_idx {
                    let key = (vi, vn_idx);
                    let new_idx = if let Some(&idxu) = vert_map.get(&key) {
                        idxu
                    } else {
                        // create new vertex in output arrays
                        let out_i = out_positions.len() as u32;
                        out_positions.push(positions[vi]);
                        if let Some(nni) = vn_idx {
                            out_normals.push(normals[nni]);
                            has_any_normals = true;
                        } else {
                            has_any_missing_normals = true;
                        }
                        vert_map.insert(key, out_i);
                        out_i
                    };
                    face_verts.push(new_idx);
                }
            }
            // triangulate
            if face_verts.len() >= 3 {
                for i in 1..(face_verts.len() - 1) {
                    indices.push(face_verts[0]);
                    indices.push(face_verts[i]);
                    indices.push(face_verts[i + 1]);
                }
            }
        }
    }

    // Build MeshData
    let normals_opt = if has_any_normals
        && !has_any_missing_normals
        && out_normals.len() == out_positions.len()
    {
        Some(out_normals)
    } else {
        None
    };
    let mesh = MeshData {
        positions: out_positions,
        normals: normals_opt,
        indices,
    };

    if mesh.positions.is_empty() || mesh.indices.is_empty() {
        anyhow::bail!("OBJ has no geometry: {}", path.as_ref().display());
    }

    Ok(mesh)
}

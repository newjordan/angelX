"""Export an actual TRELLIS GLB into the bounded AXSM1 UI roller format.

Run inside the installed TRELLIS worker (numpy/trimesh/Pillow already present):
    python export_trellis_roller.py source.glb destination-directory
No model generation, downloads, dependencies, or synthetic geometry here.
The GLB is retained separately. PCA aligns its long axle to runtime X; a
vertex-clustering reduction bounds geometry without adding a package. UVs
remain per-source vertex (texture seams are never merged across UV cells).
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import numpy as np
from PIL import Image
import trimesh


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def export(source, destination):
    out = Path(destination)
    out.mkdir(parents=True, exist_ok=True)
    scene = trimesh.load(source, force="scene", process=False)
    meshes = []
    for node in scene.graph.nodes_geometry:
        transform, name = scene.graph[node]
        mesh = scene.geometry[name].copy()
        mesh.apply_transform(transform)
        meshes.append(mesh)
    if len(meshes) != 1:
        raise ValueError(f"Expected one textured roller geometry, got {len(meshes)}; preserve source, inspect explicitly")
    mesh = meshes[0]
    positions = np.asarray(mesh.vertices, dtype=np.float64)
    faces = np.asarray(mesh.faces, dtype=np.int64)
    uv = np.asarray(mesh.visual.uv, dtype=np.float64)
    if not np.isfinite(positions).all() or not np.isfinite(uv).all():
        raise ValueError("non-finite source geometry or UVs")
    if len(uv) != len(positions) or faces.ndim != 2 or faces.shape[1] != 3:
        raise ValueError("missing UVs or nontriangular source")
    material = mesh.visual.material
    texture = getattr(material, "baseColorTexture", None)
    if texture is None:
        texture = getattr(material, "image", None)
    if texture is None:
        raise ValueError("source GLB has no base color texture")
    texture = texture.convert("RGBA")
    if max(texture.size) > 1024:
        texture.thumbnail((1024, 1024), Image.Resampling.LANCZOS)
    texture.save(out / "roller-basecolor.png")
    # PCA is only an orientation transform of the generated mesh. No geometry
    # is substituted. X=long axle, Y=up from source, Z=toward the viewer.
    centered = positions - (positions.min(0) + positions.max(0)) * 0.5
    _, vectors = np.linalg.eigh(centered.T @ centered)
    axle = vectors[:, -1]
    if axle[np.argmax(np.abs(axle))] < 0:
        axle = -axle
    up = np.array([0.0, 1.0, 0.0])
    up -= axle * np.dot(up, axle)
    if np.linalg.norm(up) < 0.01:
        up = np.array([0.0, 0.0, 1.0])
        up -= axle * np.dot(up, axle)
    up /= np.linalg.norm(up)
    forward = np.cross(axle, up)
    basis = np.stack([axle, up, forward], axis=1)
    positions = centered @ basis
    positions -= (positions.min(0) + positions.max(0)) * 0.5
    half_length = np.ptp(positions[:, 0]) * 0.5
    radial = np.linalg.norm(positions[:, 1:], axis=1)
    radius = np.quantile(radial, 0.85)
    if half_length < 1e-7 or radius < 1e-7:
        raise ValueError("degenerate generated roller")
    positions[:, 0] /= half_length
    positions[:, 1:] /= radius
    original_faces = len(faces)
    source_extents = np.ptp(positions, axis=0).tolist()
    # UV seam-preserving clustering, retaining representative generated vertices.
    # Each pass operates on the original, not on the preceding approximation.
    selected = None
    for grid in [None, 192, 128, 96, 64, 48, 32, 24, 16, 12, 8, 6, 4, 2]:
        if grid is None:
            vertices, tex, indices = positions, uv, faces
        else:
            cell = np.floor(positions * grid).astype(np.int64)
            tex_cell = np.floor(uv * 32).astype(np.int64)
            key = np.concatenate([cell, tex_cell], axis=1)
            _, representatives, inverse = np.unique(key, axis=0, return_index=True, return_inverse=True)
            vertices, tex, indices = positions[representatives], uv[representatives], inverse[faces]
            good = (indices[:, 0] != indices[:, 1]) & (indices[:, 1] != indices[:, 2]) & (indices[:, 0] != indices[:, 2])
            indices = indices[good]
        if len(indices) <= 8000 and len(vertices) <= 36000:
            selected = vertices, tex, indices
            break
    if selected is None:
        raise ValueError("source mesh cannot meet runtime bounds without a proper decimator")
    vertices, tex, indices = selected
    normals = np.zeros_like(vertices)
    a, b, c = (vertices[indices[:, i]] for i in range(3))
    face_normals = np.cross(b - a, c - a)
    nondegenerate = np.linalg.norm(face_normals, axis=1) > 1e-10
    indices = indices[nondegenerate]
    face_normals = face_normals[nondegenerate]
    for i in range(3):
        np.add.at(normals, indices[:, i], face_normals)
    magnitude = np.linalg.norm(normals, axis=1, keepdims=True)
    normals /= np.maximum(magnitude, 1e-10)
    interleaved = np.concatenate([vertices, normals, tex], axis=1).astype("<f4")
    if np.max(np.abs(vertices)) > 8 or len(indices) == 0:
        raise ValueError("generated bounds are implausible")
    target = out / "roller.axsm"
    target.write_bytes(b"AXSM0001" + struct.pack("<II", len(vertices), len(indices)) + interleaved.tobytes() + indices.astype("<u4").tobytes())
    receipt = {
        "schema": "angel-scroll-mesh-export/v1", "source": str(source), "source_sha256": sha(source),
        "source_vertices": len(positions), "source_triangles": original_faces,
        "runtime_vertices": len(vertices), "runtime_triangles": len(indices),
        "format": "AXSM0001: u32 vertices, u32 triangles; vertices=(position3,normal3,uv2) f32 LE; triangles=u32 LE",
        "basis_columns": basis.tolist(), "source_half_length": float(half_length), "source_body_radius_p85": float(radius),
        "runtime_bounds": [vertices.min(0).tolist(), vertices.max(0).tolist()],
        "source_normalized_extents": source_extents, "clustering_grid": grid,
        "radial_cross_section_ratio": float(min(source_extents[1:]) / max(source_extents[1:])),
        "texture": {"size": list(texture.size), "sha256": sha(out / "roller-basecolor.png")},
        "mesh_sha256": sha(target), "exporter_sha256": sha(__file__),
        "no_synthetic_geometry": True,
    }
    (out / "export.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source")
    parser.add_argument("destination")
    args = parser.parse_args()
    export(args.source, args.destination)

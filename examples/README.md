# Example Meshes

These STL files are intentionally small, human-readable examples for understanding `meshlint` findings. GitHub renders `.stl` files directly, so click any mesh link below to inspect the shape in the repository UI.

Run an example locally with:

```bash
cargo run -p meshlint-cli -- examples/meshes/open-boundary-triangle.stl --process sla
```

| Mesh | Primary finding | What to look for |
| --- | --- | --- |
| [open-boundary-triangle.stl](meshes/open-boundary-triangle.stl) | `mesh/boundary-loop` | A single open triangle with every edge exposed. |
| [non-manifold-extra-face.stl](meshes/non-manifold-extra-face.stl) | `mesh/non-manifold` | A tetrahedron with an extra face sharing one edge, so that edge belongs to three faces. |
| [inconsistent-normals-tetra.stl](meshes/inconsistent-normals-tetra.stl) | `mesh/inconsistent-normals` | A closed tetrahedron with one face wound in the wrong direction. |
| [inverted-normals-tetra.stl](meshes/inverted-normals-tetra.stl) | `mesh/inverted-normals` | A closed tetrahedron with every face wound inward. |
| [tiny-shell.stl](meshes/tiny-shell.stl) | `mesh/tiny-shell` | A normal tetrahedron plus a tiny detached tetrahedron. |
| [disconnected-shell.stl](meshes/disconnected-shell.stl) | `mesh/disconnected-shell` | Two separate tetrahedra large enough to be treated as separate model parts. |
| [self-intersection.stl](meshes/self-intersection.stl) | `mesh/self-intersection` | Two triangles crossing through each other. |
| [sla-large-cross-section.stl](meshes/sla-large-cross-section.stl) | `sla/large-cross-section` | A flat 50 mm x 50 mm plate with a large peel area for SLA printing. |

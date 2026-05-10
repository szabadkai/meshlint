# Issue Reference

This page documents every issue `meshlint` can currently report, plus the automatic fixes that can be applied with `--fix`.

`meshlint` reports findings in an ESLint-style format:

```text
model.stl
  ERROR: edge 12-44 is shared by 1 faces; expected 2 (face 91) [mesh/non-manifold]
  WARN: shell 7 has 4 faces and is below 1.000 mm3 (faces 320, 321, 322, 323) [mesh/tiny-shell]
```

The bracketed value is the rule id. JSON output uses the same `rule_id` values.

## Severities

### `ERROR`

The mesh has a structural defect that can make the model ambiguous, unprintable, or unsafe to process downstream. The CLI exits with code `1` when any error remains.

### `WARN`

The mesh has a likely problem, but it may still be intentional or printable depending on the model and process.

### `INFO`

Context or summary information. Informational findings do not affect the exit code.

## Findings

### `*-truncated`

Severity: `INFO`

Example:

```text
INFO: 7992 additional mesh/tiny-edge findings omitted after limit 100 [mesh/tiny-edge-truncated]
```

What it means:

`meshlint` found more findings for a rule than the configured per-rule output limit.

Why it matters:

Large meshes can contain thousands of repeated low-level defects. Truncation keeps CLI and JSON output usable while preserving a count of omitted findings.

Recommended action:

Increase or disable the cap when you need every finding:

```bash
meshlint model.stl --max-findings-per-rule 5000
meshlint model.stl --max-findings-per-rule 0
```

JSON fields:

- `metrics.omitted`: omitted finding count
- `metrics.limit`: configured per-rule limit

### `mesh/degenerate-face`

Severity: `ERROR`

Example:

```text
ERROR: face 128 is degenerate (face 128) [mesh/degenerate-face]
```

What it means:

A triangle has repeated vertices or zero area.

Common cases:

- Duplicate vertices collapsed a triangle into a line or point.
- Exporter or remesher emitted zero-area triangles.
- Floating-point cleanup left invalid faces behind.

Why it matters:

Degenerate triangles do not describe real surface area and can confuse topology, normal, volume, and slicing calculations.

Current `--fix` behavior:

Fixable. `--fix` removes degenerate faces.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

JSON fields:

- `face_ids`: the degenerate face
- `metrics.area_mm2`: computed triangle area
- `metrics.repeated_vertex`: `1` when the face repeats a vertex, otherwise `0`

### `mesh/duplicate-face`

Severity: `WARN`

Example:

```text
WARN: face 22 duplicates face 9 (faces 9, 22) [mesh/duplicate-face]
```

What it means:

Two triangles use the same three vertices, regardless of winding order.

Common cases:

- Duplicate surfaces from export.
- Boolean operations that left coincident faces.
- Manual mesh edits that copied geometry in place.

Why it matters:

Duplicate faces can make edges appear non-manifold and can confuse slicers, normals, and volume estimates.

Current `--fix` behavior:

Fixable. `--fix` removes duplicate faces.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

JSON fields:

- `face_ids`: original face and duplicate face
- `metrics.original_face`: first face with this vertex set
- `metrics.duplicate_face`: duplicate face id

### `mesh/tiny-edge`

Severity: `WARN`

Example:

```text
WARN: edge 0-1 is 0.00100 mm; below 0.01000 mm (face 0) [mesh/tiny-edge]
```

What it means:

An edge is shorter than the configured tiny-edge threshold.

Common cases:

- Noisy scan or remesh output.
- Nearly collapsed triangles.
- CAD exports with tiny sliver features.
- Geometry that should be welded or simplified.

Why it matters:

Very small edges can destabilize boolean operations, wall-thickness checks, self-intersection checks, and slicer repair heuristics. They are also a common source of skinny triangles.

Current `--fix` behavior:

Partially fixable. If the edge is caused by near-duplicate vertices, `--fix` may remove it through vertex welding and degenerate-face removal. It does not currently remesh arbitrary tiny edges.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

Tune the threshold when needed:

```bash
meshlint model.stl --tiny-edge 0.005
```

JSON fields:

- `face_ids`: faces touching the tiny edge
- `metrics.vertex_a`: first edge vertex id
- `metrics.vertex_b`: second edge vertex id
- `metrics.length_mm`: measured edge length
- `metrics.threshold_mm`: threshold used for the check

Example mesh:

- [tiny-edge.stl](../examples/meshes/tiny-edge.stl)

### `mesh/bad-aspect-triangle`

Severity: `WARN`

Example:

```text
WARN: face 0 has aspect ratio 1000.00; threshold is 100.00 (face 0) [mesh/bad-aspect-triangle]
```

What it means:

A triangle's longest edge is much longer than its shortest edge.

Common cases:

- Sliver triangles from boolean operations.
- Decimation artifacts.
- Scan cleanup artifacts.
- CAD tessellation with poor local triangle quality.

Why it matters:

Skinny triangles can make geometric predicates numerically fragile. They also make ray casts, distance checks, and local feature analysis less reliable.

Current `--fix` behavior:

Not directly auto-fixable. `--fix` can remove the worst cases when welding turns them into degenerate faces, but general repair requires remeshing.

Recommended action:

Remesh or simplify the local region if the finding appears near printability defects or failed geometry operations.

JSON fields:

- `face_ids`: the bad-aspect face
- `metrics.aspect_ratio`: longest edge divided by shortest edge
- `metrics.threshold`: configured threshold
- `metrics.shortest_edge_mm`: shortest edge length
- `metrics.longest_edge_mm`: longest edge length

Example mesh:

- [bad-aspect-triangle.stl](../examples/meshes/bad-aspect-triangle.stl)

### `mesh/non-manifold`

Severity: `ERROR`

Example:

```text
ERROR: edge 44-45 is shared by 3 faces; expected 2 (faces 91, 104, 118) [mesh/non-manifold]
```

What it means:

A watertight triangle mesh should normally have every edge shared by exactly two faces. This rule reports each edge where that is not true.

Common cases:

- An edge belongs to only one face, which usually means there is an open boundary or hole.
- An edge belongs to three or more faces, which usually means duplicate/internal faces, self-intersection, or joined geometry that does not form a clean surface.
- Vertices that should be identical are slightly separated, causing adjacent triangles to appear disconnected.

Why it matters:

Non-manifold geometry can confuse slicers, hollowing tools, support generation, boolean operations, and resin trap detection. For printing, it can produce missing walls, unexpected filled regions, or invalid toolpaths.

Current `--fix` behavior:

Partially fixable. `--fix` can often reduce or eliminate this issue when the cause is near-duplicate vertices, duplicate faces, degenerate faces, tiny loose shells, or inconsistent winding. It does not yet fill holes or resolve self-intersections.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

If the issue remains, inspect the reported face ids around the bad edge and repair the source model or mesh in a modeling tool.

JSON fields:

- `face_ids`: faces touching the bad edge
- `metrics.vertex_a`: first edge vertex id
- `metrics.vertex_b`: second edge vertex id
- `metrics.faces`: number of faces using the edge

Example meshes:

- [open-boundary-triangle.stl](../examples/meshes/open-boundary-triangle.stl)
- [non-manifold-extra-face.stl](../examples/meshes/non-manifold-extra-face.stl)

### `mesh/boundary-loop`

Severity: `ERROR`

Example:

```text
ERROR: boundary loop 0 has 18 edges; mesh surface is open (faces 91, 92, 93, 94, +6 more) [mesh/boundary-loop]
```

What it means:

The mesh has a connected component of boundary edges. A boundary edge is an edge used by exactly one face. In a watertight printable mesh, these edges usually form the rim of a hole or open surface.

Common cases:

- A hole in the model.
- An open surface exported as STL.
- Adjacent triangles whose vertices are not welded.
- Missing faces after a boolean, decimation, or scan cleanup operation.

Why it matters:

Individual `mesh/non-manifold` edge findings are precise, but a real hole may produce hundreds or thousands of one-face edge errors. This rule groups connected boundary edges into one actionable defect so users can find the open region.

Current `--fix` behavior:

Partially fixable. If the boundary exists because nearby vertices are not welded, `--fix` may close it through `fix/weld-vertices`. If it is a real hole or open surface, `--fix` does not fill it yet.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

If the boundary remains, fill the hole or repair the open surface in the source model. Future versions may add safe filling for simple planar holes.

JSON fields:

- `face_ids`: faces adjacent to the boundary component
- `metrics.component`: boundary component index
- `metrics.edges`: number of boundary edges in the component
- `metrics.vertices`: number of vertices in the component
- `metrics.closed`: `1` when every boundary vertex has degree 2, otherwise `0`

Example mesh:

- [open-boundary-triangle.stl](../examples/meshes/open-boundary-triangle.stl)

### `mesh/inconsistent-normals`

Severity: `WARN`

Example:

```text
WARN: faces 12 and 13 traverse shared edge 40-41 in the same direction (faces 12, 13) [mesh/inconsistent-normals]
```

What it means:

Two adjacent faces that share an edge should traverse that edge in opposite directions. If both faces traverse the shared edge in the same direction, their winding is inconsistent.

Common cases:

- Some triangles are flipped.
- A shell has mixed inward and outward normals.
- Exported STL facet normals disagree with triangle winding.

Why it matters:

Slicers and repair tools often infer inside/outside from winding. Inconsistent winding can cause inverted regions, hollowing mistakes, incorrect volume estimates, and unreliable support or cavity analysis.

Current `--fix` behavior:

Fixable. `--fix` traverses face adjacency and rewinds faces so neighboring triangles agree. If the total signed volume is negative, it flips the whole mesh outward.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

Then re-run linting on the fixed STL.

JSON fields:

- `face_ids`: the two adjacent faces with conflicting winding
- `metrics.vertex_a`: first shared edge vertex id
- `metrics.vertex_b`: second shared edge vertex id

Example mesh:

- [inconsistent-normals-tetra.stl](../examples/meshes/inconsistent-normals-tetra.stl)

### `mesh/inverted-normals`

Severity: `WARN`

Example:

```text
WARN: mesh winding appears globally inverted [mesh/inverted-normals]
```

What it means:

The mesh has negative signed volume, which usually means the shell is wound inward instead of outward.

Common cases:

- Exporter flipped face winding.
- Boolean operation inverted a shell.
- A mesh was mirrored without correcting normals.

Why it matters:

Inside/outside classification can be reversed. That can break hollowing, support placement, resin-trap analysis, and slicer repair heuristics.

Current `--fix` behavior:

Fixable. `--fix` flips global winding when signed volume is negative.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

JSON fields:

- `metrics.signed_volume_mm3`: signed volume estimate

Example mesh:

- [inverted-normals-tetra.stl](../examples/meshes/inverted-normals-tetra.stl)

### `mesh/zero-volume-shell`

Severity: `WARN`

Example:

```text
WARN: shell 0 has surface area but near-zero enclosed volume (faces 0, 1) [mesh/zero-volume-shell]
```

What it means:

A connected shell has nonzero surface area but near-zero signed enclosed volume.

Common cases:

- Open sheets.
- Duplicate opposite faces.
- Collapsed geometry.
- Surfaces that look visible but do not define a solid.

Why it matters:

Zero-volume shells are not printable solids. They often break hollowing, volume estimates, support logic, and wall-thickness checks.

Current `--fix` behavior:

Partially fixable. If the zero-volume shell is tiny, `--fix` may remove it through tiny-shell cleanup. If it is caused by duplicate faces, `--fix` can remove exact duplicates. It does not currently reconstruct solid volume from open sheets.

Recommended action:

Remove sheet-like geometry or give it real thickness in the source model.

JSON fields:

- `face_ids`: faces in the zero-volume shell
- `metrics.shell`: shell index
- `metrics.faces`: face count
- `metrics.volume_mm3`: signed-volume magnitude
- `metrics.surface_area_mm2`: surface area estimate

Example mesh:

- [zero-volume-shell.stl](../examples/meshes/zero-volume-shell.stl)

### `mesh/unit-suspicious`

Severity: `WARN`

Example:

```text
WARN: model maximum dimension is 0.100 mm; units may be too small [mesh/unit-suspicious]
```

What it means:

The model's bounding box is outside the configured expected size range.

Common cases:

- Inches exported as millimeters.
- Meters exported as millimeters.
- A normalized asset exported without applying scale.
- A miniature or engineering detail intentionally outside default thresholds.

Why it matters:

Wrong units can make every printability threshold meaningless. A model that should be 100 mm wide may appear as 3.94 mm or 0.1 mm depending on export scale.

Current `--fix` behavior:

Not auto-fixable. Scaling is a design-intent decision.

Recommended action:

Confirm the model units and rescale intentionally if needed. Tune the thresholds:

```bash
meshlint model.stl --suspicious-min-dimension 0.1 --suspicious-max-dimension 2000
```

JSON fields:

- `metrics.x_mm`: X dimension
- `metrics.y_mm`: Y dimension
- `metrics.z_mm`: Z dimension
- `metrics.max_dimension_mm`: largest dimension
- `metrics.min_threshold_mm`: lower threshold
- `metrics.max_threshold_mm`: upper threshold

Example mesh:

- [unit-suspicious-small.stl](../examples/meshes/unit-suspicious-small.stl)

### `mesh/self-intersection`

Severity: `ERROR`

Example:

```text
ERROR: faces 120 and 844 intersect (faces 120, 844) [mesh/self-intersection]
```

What it means:

Two non-adjacent triangles cross through each other.

Common cases:

- Failed boolean operations.
- Overlapping parts exported as one mesh.
- Sculpted or scanned surfaces that pass through themselves.
- Shells pushed through each other during editing.

Why it matters:

Self-intersections make inside/outside ambiguous and can cause slicers to fill, erase, or reinterpret regions unpredictably.

Current `--fix` behavior:

Not safely auto-fixable. The current implementation reports the intersecting face pair but does not modify the mesh.

Recommended action:

Repair the source model with boolean cleanup, remeshing, or manual surface edits.

JSON fields:

- `face_ids`: the two intersecting faces
- `metrics.face_a`: first intersecting face
- `metrics.face_b`: second intersecting face

Example mesh:

- [self-intersection.stl](../examples/meshes/self-intersection.stl)

### `mesh/self-intersection-skipped`

Severity: `WARN`

Example:

```text
WARN: self-intersection check skipped after 50001 candidate tests; limit is 50000 [mesh/self-intersection-skipped]
```

What it means:

The model is dense enough that the bounded self-intersection pass reached its configured triangle-pair test budget.

Why it matters:

Without a budget, self-intersection checks can dominate runtime on large STL files. This warning means `meshlint` returned a bounded result instead of spending unbounded time on pairwise geometry tests.

Current `--fix` behavior:

Not fixable. This is a runtime guard, not a mesh defect.

Recommended action:

For a faster normal lint, keep the default. For deeper analysis, increase the budget:

```bash
meshlint model.stl --max-self-intersection-tests 500000
```

JSON fields:

- `metrics.faces`: face count
- `metrics.attempted_tests`: candidate triangle tests attempted
- `metrics.max_tests`: configured test budget

### `mesh/tiny-shell`

Severity: `WARN`

Example:

```text
WARN: shell 7 has 4 faces and is below 1.000 mm3 (faces 320, 321, 322, 323) [mesh/tiny-shell]
```

What it means:

The mesh contains a disconnected shell whose estimated size is below the configured tiny-shell threshold. `meshlint` keeps the primary shell out of this rule and reports only extra loose shells.

Common cases:

- Stray triangles left by a boolean operation.
- Small chips or fragments from scanning/remeshing.
- Unwanted debris from CAD export.
- Tiny decorative features that are below the intended print process resolution.

Why it matters:

Tiny shells can create confusing slicer artifacts, hard-to-clean resin debris, or meaningless islands. They also make topology reports noisy because each tiny shell often contributes several non-manifold edges.

Current `--fix` behavior:

Fixable. `--fix` removes tiny extra shells below `--tiny-shell-volume`. The default threshold is `1.0` mm3.

Recommended action:

Run:

```bash
meshlint model.stl --fix
```

Tune the threshold when needed:

```bash
meshlint model.stl --fix --tiny-shell-volume 0.1
```

JSON fields:

- `face_ids`: faces in the tiny shell
- `metrics.shell`: shell index
- `metrics.faces`: face count in the shell
- `metrics.size_estimate_mm3`: estimated shell size
- `metrics.threshold_mm3`: threshold used for the check

Example mesh:

- [tiny-shell.stl](../examples/meshes/tiny-shell.stl)

### `mesh/disconnected-shell`

Severity: `WARN`

Example:

```text
WARN: shell 3 is disconnected from the primary shell and has 812 faces (faces 2048, 2049, 2050, 2051, +808 more) [mesh/disconnected-shell]
```

What it means:

The model contains a disconnected shell that is large enough not to be classified as tiny. A shell is a connected set of faces. The primary shell is the largest shell by estimated size.

Common cases:

- A multi-part model exported as one STL.
- Separate supports, pins, inserts, or accessories.
- Accidental floating geometry.
- A failed boolean operation that left a large loose component.

Why it matters:

Disconnected shells may be intentional, but they are often surprising in printing workflows. For SLA, loose shells can become unsupported islands. For FDM, they may produce isolated extrusions. For repair workflows, they can hide topology defects.

Current `--fix` behavior:

Not automatically removed unless the shell is below `--tiny-shell-volume`. Large disconnected shells may be intentional, so `--fix` leaves them intact.

Recommended action:

Inspect whether the shell is intentional. If it is debris, remove it in the source model or lower-level mesh editor. If it is intentional, keep it and treat the warning as informational for your workflow.

JSON fields:

- `face_ids`: faces in the disconnected shell
- `metrics.shell`: shell index
- `metrics.faces`: face count in the shell
- `metrics.size_estimate_mm3`: estimated shell size
- `metrics.threshold_mm3`: tiny-shell threshold used for classification

Example mesh:

- [disconnected-shell.stl](../examples/meshes/disconnected-shell.stl)

### `mesh/disconnected-shells-summary`

Severity: `INFO`

Example:

```text
INFO: mesh contains 5 disconnected shells [mesh/disconnected-shells-summary]
```

What it means:

This is a summary emitted when the mesh has more than one shell. Individual loose shells are still reported separately as `mesh/tiny-shell` or `mesh/disconnected-shell`.

Why it matters:

The summary gives a quick count while preserving ESLint-style per-defect findings.

Current `--fix` behavior:

Indirectly affected. If `--fix` removes tiny shells or welds separated components back together, this summary may disappear.

JSON fields:

- `metrics.shells`: total shell count

Example meshes:

- [tiny-shell.stl](../examples/meshes/tiny-shell.stl)
- [disconnected-shell.stl](../examples/meshes/disconnected-shell.stl)

### `sla/large-cross-section`

Severity: `WARN`

Example:

```text
WARN: large cross-section near layer 428 is 1440.20 mm2; threshold is 1200.00 mm2 (faces 1024, 1025, 1026, 1027, +42 more) [sla/large-cross-section]
```

What it means:

For SLA profiles, `meshlint` estimates projected XY area per layer and reports layers whose area exceeds the configured threshold.

Common cases:

- Large flat faces parallel to the build plate.
- A model oriented with its widest side horizontal.
- Thick solid sections that create high peel force.

Why it matters:

Large cross-sections can increase peel force, create suction stress, and raise the chance of print failure on resin printers.

Current `--fix` behavior:

Not auto-fixable. This is usually solved by reorienting the model, hollowing, adding drainage/venting, or changing support strategy.

Recommended action:

Try a different orientation or reduce large flat peel areas. Tune the threshold and layer height:

```bash
meshlint model.stl --process sla --large-cross-section 900 --layer-height 0.05
```

JSON fields:

- `face_ids`: faces contributing to the layer estimate
- `metrics.layer`: estimated layer index
- `metrics.z_mm`: estimated Z position
- `metrics.area_mm2`: estimated projected area
- `metrics.threshold_mm2`: configured threshold

Example mesh:

- [sla-large-cross-section.stl](../examples/meshes/sla-large-cross-section.stl)

## Fixes

These are not findings. They appear under `fixes` in JSON and as `FIXED:` lines in text output when `--fix` changes the mesh.

### `fix/weld-vertices`

Example:

```text
FIXED: merged 12482 vertices within 0.0100 mm
```

What it does:

Merges vertices that are within the configured weld tolerance.

Default tolerance:

```text
0.01 mm
```

CLI option:

```bash
meshlint model.stl --fix --weld-tolerance 0.005
```

Useful for:

- STL triangle soup where every triangle has its own vertex copies.
- Nearly identical vertices caused by floating-point export noise.
- Non-manifold reports caused by tiny gaps between adjacent triangles.

Risk:

Too large a weld tolerance can collapse small intended features. Keep this value small relative to the model's detail size.

### `fix/remove-degenerate-faces`

Example:

```text
FIXED: removed 12 degenerate faces
```

What it does:

Removes triangles with repeated vertices or zero area.

Useful for:

- Bad exports.
- Meshes after welding duplicate vertices.
- Broken scanner/remesher output.

Risk:

Low. Degenerate triangles do not describe printable surface area.

### `fix/remove-duplicate-faces`

Example:

```text
FIXED: removed 42 duplicate faces
```

What it does:

Removes exact duplicate triangles, regardless of winding order.

Useful for:

- Duplicate surfaces from exports or boolean operations.
- Edges incorrectly used by more than two faces.

Risk:

Low for exact duplicates. It does not remove merely overlapping or intersecting faces unless they use the same vertices.

### `fix/orient-faces`

Example:

```text
FIXED: rewound 842 faces for consistent normals
```

What it does:

Traverses connected faces and flips winding where neighboring faces disagree. If the mesh appears globally inverted by signed volume, it flips all faces.

Useful for:

- Mixed face winding.
- Inconsistent normals.
- STL files where facet normals are unreliable but vertex winding can be repaired.

Risk:

Moderate on highly non-manifold meshes. The traversal is local and deterministic, but ambiguous topology can still make "inside" and "outside" unclear.

### `fix/remove-tiny-shells`

Example:

```text
FIXED: removed 120 faces from tiny disconnected shells
```

What it does:

Removes disconnected shells below `--tiny-shell-volume`, while preserving the primary shell.

Default threshold:

```text
1.0 mm3
```

CLI option:

```bash
meshlint model.stl --fix --tiny-shell-volume 0.25
```

Useful for:

- Removing floating debris.
- Cleaning scan artifacts.
- Reducing noisy topology errors caused by tiny loose fragments.

Risk:

Moderate. Tiny shells can be intentional details. Lower the threshold if the model intentionally contains small detached features.

## Planned Findings

These checks are part of the product direction but are not implemented yet:

- `print/thin-wall`
- `print/narrow-pin`
- `print/unsupported-island`
- `sla/resin-trap`
- `sla/suction-cup`
- `print/fragile-feature`

They should remain out of release notes and rule docs until they produce real findings.

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
- `sla/large-cross-section`
- `print/fragile-feature`

They should remain out of release notes and rule docs until they produce real findings.

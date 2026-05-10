# meshlint

Like ESLint, but for 3D models.

`meshlint` is a CLI-first mesh linter with a WASM-ready core. The first implementation focuses on STL hygiene checks and safe automatic repairs:

- non-manifold edges
- boundary loops / open boundary components
- degenerate faces
- duplicate faces
- tiny edges
- bad-aspect triangles
- inconsistent face winding / normals
- inverted normals
- self-intersections
- zero-volume shells
- suspicious units / scale
- SLA large cross-sections
- disconnected shells
- tiny disconnected shells
- duplicate vertices
- near-duplicate vertices within weld tolerance
- consistent face orientation

## CLI

```bash
cargo run -p meshlint-cli -- model.stl --process sla
cargo run -p meshlint-cli -- model.stl --process sla --fix
cargo run -p meshlint-cli -- model.stl --process sla --fix --out model.fixed.stl
cargo run -p meshlint-cli -- model.stl --format json
cargo run -p meshlint-cli -- model.stl --max-findings-per-rule 100
cargo run -p meshlint-cli -- model.stl --max-self-intersection-tests 500000
```

`meshlint` also reads an optional JSON config file from `~/.meshlintrc`. The config uses the same option shape as the core API, and any CLI flags you pass override values from the file:

```json
{
  "process": "sla",
  "thresholds": {
    "wall_min_mm": 0.8,
    "tiny_shell_volume_mm3": 1.0,
    "weld_tolerance_mm": 0.01,
    "layer_height_mm": 0.05,
    "large_cross_section_mm2": 1200,
    "tiny_edge_mm": 0.01,
    "bad_aspect_ratio": 100,
    "suspicious_min_dimension_mm": 0.5,
    "suspicious_max_dimension_mm": 1000,
    "max_self_intersection_tests": 50000,
    "max_findings_per_rule": 1000
  }
}
```

All fields are optional. Missing values use the built-in defaults.

Text output is intentionally ESLint-like:

```text
model.stl
  FIXED: merged 12482 vertices within 0.0100 mm
  FIXED: rewound 842 faces for consistent normals
  SUMMARY: errors 1 -> 0, warnings 2 -> 0
  ERROR: edge 12-44 is shared by 1 faces; expected 2 (face 91) [mesh/non-manifold]
  ERROR: boundary loop 0 has 18 edges; mesh surface is open (faces 91, 92, 93, 94, +6 more) [mesh/boundary-loop]
  ERROR: edge 44-45 is shared by 3 faces; expected 2 (faces 91, 104, 118) [mesh/non-manifold]
  ERROR: faces 120 and 844 intersect (faces 120, 844) [mesh/self-intersection]
  WARN: shell 7 has 4 faces and is below 1.000 mm3 (faces 320, 321, 322, 323) [mesh/tiny-shell]
  WARN: large cross-section near layer 428 is 1440.20 mm2; threshold is 1200.00 mm2 [sla/large-cross-section]
  INFO: mesh contains 2 disconnected shells [mesh/disconnected-shells-summary]

Wrote: model.fixed.stl
```

Each topological defect is emitted as its own finding in text and JSON. Large meshes can produce many lines; this is intentional so the output can be consumed like a linter report.

See [docs/issues.md](docs/issues.md) for the full issue and fix reference, including fixture meshes that demonstrate common findings.

Exit codes:

- `0`: no errors
- `1`: lint errors found
- `2`: invalid input, unsupported format, or runtime failure

Performance guards:

- `--max-findings-per-rule`: caps noisy per-defect output per rule. Default: `1000`. Use `0` for no cap.
- `--max-self-intersection-tests`: caps expensive triangle-pair self-intersection tests. Default: `50000`. Increase for deeper checks on dense meshes.

## WASM

The geometry engine lives in `crates/meshlint-core` and does not use filesystem APIs. The browser wrapper is in `crates/meshlint-wasm`.

```bash
wasm-pack build crates/meshlint-wasm --target web
```

Expected browser API:

```ts
import init, { lintMesh, fixMesh } from "./pkg/meshlint_wasm.js";

await init();

const report = lintMesh(bytes, "stl", {
  process: "sla",
  thresholds: {
    wall_min_mm: 0.8,
    tiny_shell_volume_mm3: 1.0,
    weld_tolerance_mm: 0.01,
    layer_height_mm: 0.05,
    large_cross_section_mm2: 1200,
    tiny_edge_mm: 0.01,
    bad_aspect_ratio: 100,
    suspicious_min_dimension_mm: 0.5,
    suspicious_max_dimension_mm: 1000,
    max_self_intersection_tests: 50000,
    max_findings_per_rule: 1000
  }
});

const fixed = fixMesh(bytes, "stl", { process: "sla" });
```

`fixMesh` returns findings, applied fixes, and `fixed_bytes` containing a binary STL.

## Workspace

```text
crates/
  meshlint-core/   pure mesh parser, linter, and fixer
  meshlint-cli/    native command-line interface
  meshlint-wasm/   wasm-bindgen wrapper
```

## Near-Term Roadmap

- add wall-thickness estimates
- add narrow pin / fragile feature checks
- add SLA resin-trap and suction-cup checks
- add layer cross-section analysis
- add visual debug export for highlighted findings
- add OBJ and 3MF input

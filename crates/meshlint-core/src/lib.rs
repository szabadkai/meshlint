use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, thiserror::Error)]
pub enum MeshLintError {
    #[error("unsupported mesh format: {0}")]
    UnsupportedFormat(String),
    #[error("invalid STL: {0}")]
    InvalidStl(String),
    #[error("mesh is too large for binary STL output")]
    MeshTooLarge,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LintOptions {
    #[serde(default = "default_process")]
    pub process: String,
    #[serde(default)]
    pub thresholds: Thresholds,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            process: default_process(),
            thresholds: Thresholds::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thresholds {
    #[serde(default = "default_wall_min_mm")]
    pub wall_min_mm: f32,
    #[serde(default = "default_tiny_shell_volume_mm3")]
    pub tiny_shell_volume_mm3: f32,
    #[serde(default = "default_weld_tolerance_mm")]
    pub weld_tolerance_mm: f32,
    #[serde(default = "default_layer_height_mm")]
    pub layer_height_mm: f32,
    #[serde(default = "default_large_cross_section_mm2")]
    pub large_cross_section_mm2: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            wall_min_mm: default_wall_min_mm(),
            tiny_shell_volume_mm3: default_tiny_shell_volume_mm3(),
            weld_tolerance_mm: default_weld_tolerance_mm(),
            layer_height_mm: default_layer_height_mm(),
            large_cross_section_mm2: default_large_cross_section_mm2(),
        }
    }
}

fn default_process() -> String {
    "sla".to_string()
}

fn default_wall_min_mm() -> f32 {
    0.8
}

fn default_tiny_shell_volume_mm3() -> f32 {
    1.0
}

fn default_weld_tolerance_mm() -> f32 {
    0.01
}

fn default_layer_height_mm() -> f32 {
    0.05
}

fn default_large_cross_section_mm2() -> f32 {
    1200.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

type Edge = (u32, u32);
type FaceEdge = (u32, Edge);

#[derive(Clone, Debug)]
struct BoundaryEdge {
    edge: Edge,
    face_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub face_ids: Vec<u32>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metrics: HashMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fix {
    pub rule_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metrics: HashMap<String, f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LintReport {
    pub process: String,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixReport {
    pub process: String,
    pub initial_findings: Vec<Finding>,
    pub fixes: Vec<Fix>,
    pub findings: Vec<Finding>,
    pub fixed_bytes: Vec<u8>,
}

pub fn lint_mesh_bytes(
    bytes: &[u8],
    format: &str,
    options: LintOptions,
) -> Result<LintReport, MeshLintError> {
    let mesh = parse_mesh(bytes, format)?;
    Ok(LintReport {
        process: options.process.clone(),
        findings: lint_mesh(&mesh, &options),
    })
}

pub fn fix_mesh_bytes(
    bytes: &[u8],
    format: &str,
    options: LintOptions,
) -> Result<FixReport, MeshLintError> {
    if !format.eq_ignore_ascii_case("stl") {
        return Err(MeshLintError::UnsupportedFormat(format.to_string()));
    }

    let mut mesh = parse_stl(bytes)?;
    let initial_findings = lint_mesh(&mesh, &options);
    let fixes = fix_mesh(&mut mesh, &options);
    let findings = lint_mesh(&mesh, &options);
    let fixed_bytes = write_binary_stl(&mesh)?;

    Ok(FixReport {
        process: options.process,
        initial_findings,
        fixes,
        findings,
        fixed_bytes,
    })
}

pub fn lint_mesh(mesh: &Mesh, options: &LintOptions) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(check_degenerate_faces(mesh));
    findings.extend(check_duplicate_faces(mesh));
    findings.extend(check_non_manifold(mesh));
    findings.extend(check_boundary_loops(mesh));
    findings.extend(check_normal_consistency(mesh));
    findings.extend(check_inverted_normals(mesh));
    findings.extend(check_self_intersections(mesh));
    findings.extend(check_shells(mesh, options.thresholds.tiny_shell_volume_mm3));
    if options.process.eq_ignore_ascii_case("sla") {
        findings.extend(check_large_cross_sections(mesh, &options.thresholds));
    }
    findings
}

pub fn fix_mesh(mesh: &mut Mesh, options: &LintOptions) -> Vec<Fix> {
    let mut fixes = Vec::new();
    fixes.extend(weld_vertices(mesh, options.thresholds.weld_tolerance_mm));
    fixes.extend(remove_degenerate_faces(mesh));
    fixes.extend(remove_duplicate_faces(mesh));
    fixes.extend(orient_faces_consistently(mesh));
    fixes.extend(remove_tiny_shells(
        mesh,
        options.thresholds.tiny_shell_volume_mm3,
    ));
    fixes
}

fn parse_mesh(bytes: &[u8], format: &str) -> Result<Mesh, MeshLintError> {
    match format.to_ascii_lowercase().as_str() {
        "stl" => parse_stl(bytes),
        other => Err(MeshLintError::UnsupportedFormat(other.to_string())),
    }
}

pub fn parse_stl(bytes: &[u8]) -> Result<Mesh, MeshLintError> {
    if looks_like_binary_stl(bytes) {
        parse_binary_stl(bytes)
    } else {
        parse_ascii_stl(bytes)
    }
}

fn looks_like_binary_stl(bytes: &[u8]) -> bool {
    if bytes.len() < 84 {
        return false;
    }
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    84 + count.saturating_mul(50) == bytes.len()
}

fn parse_binary_stl(bytes: &[u8]) -> Result<Mesh, MeshLintError> {
    if bytes.len() < 84 {
        return Err(MeshLintError::InvalidStl(
            "binary header is truncated".into(),
        ));
    }
    let triangle_count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    let expected_len = 84 + triangle_count.saturating_mul(50);
    if bytes.len() != expected_len {
        return Err(MeshLintError::InvalidStl(
            "binary triangle data is truncated".into(),
        ));
    }

    let mut vertices = Vec::with_capacity(triangle_count * 3);
    let mut faces = Vec::with_capacity(triangle_count);
    let mut vertex_ids = HashMap::new();
    let mut offset = 84;
    for _ in 0..triangle_count {
        offset += 12;
        let mut face = [0u32; 3];
        for vertex_slot in &mut face {
            let x = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let y = f32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            let z = f32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            let vertex_index = add_vertex(&mut vertices, &mut vertex_ids, [x, y, z]);
            *vertex_slot = vertex_index;
            offset += 12;
        }
        faces.push(face);
        offset += 2;
    }

    Ok(Mesh { vertices, faces })
}

fn parse_ascii_stl(bytes: &[u8]) -> Result<Mesh, MeshLintError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| MeshLintError::InvalidStl("ASCII STL is not valid UTF-8".into()))?;
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut vertex_ids = HashMap::new();
    let mut pending = Vec::with_capacity(3);

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("vertex") {
            continue;
        }
        let x = parse_f32(parts.next(), "x")?;
        let y = parse_f32(parts.next(), "y")?;
        let z = parse_f32(parts.next(), "z")?;
        pending.push(add_vertex(&mut vertices, &mut vertex_ids, [x, y, z]));
        if pending.len() == 3 {
            faces.push([pending[0], pending[1], pending[2]]);
            pending.clear();
        }
    }

    if faces.is_empty() {
        return Err(MeshLintError::InvalidStl("no triangles found".into()));
    }

    Ok(Mesh { vertices, faces })
}

fn add_vertex(
    vertices: &mut Vec<[f32; 3]>,
    vertex_ids: &mut HashMap<(u32, u32, u32), u32>,
    vertex: [f32; 3],
) -> u32 {
    let key = vertex_key(vertex);
    *vertex_ids.entry(key).or_insert_with(|| {
        let id = vertices.len() as u32;
        vertices.push(vertex);
        id
    })
}

fn vertex_key(vertex: [f32; 3]) -> (u32, u32, u32) {
    (
        normalized_f32_bits(vertex[0]),
        normalized_f32_bits(vertex[1]),
        normalized_f32_bits(vertex[2]),
    )
}

fn normalized_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

fn parse_f32(value: Option<&str>, axis: &str) -> Result<f32, MeshLintError> {
    value
        .ok_or_else(|| MeshLintError::InvalidStl(format!("missing {axis} coordinate")))?
        .parse::<f32>()
        .map_err(|_| MeshLintError::InvalidStl(format!("invalid {axis} coordinate")))
}

pub fn write_binary_stl(mesh: &Mesh) -> Result<Vec<u8>, MeshLintError> {
    let triangle_count =
        u32::try_from(mesh.faces.len()).map_err(|_| MeshLintError::MeshTooLarge)?;
    let mut out = Vec::with_capacity(84 + mesh.faces.len() * 50);
    let mut header = [0u8; 80];
    header[..22].copy_from_slice(b"meshlint fixed binary ");
    out.extend_from_slice(&header);
    out.extend_from_slice(&triangle_count.to_le_bytes());

    for face in &mesh.faces {
        let normal = face_normal(mesh, *face);
        for value in normal {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for vertex_id in face {
            for value in mesh.vertices[*vertex_id as usize] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }

    Ok(out)
}

fn check_degenerate_faces(mesh: &Mesh) -> Vec<Finding> {
    mesh.faces
        .iter()
        .enumerate()
        .filter_map(|(face_id, face)| {
            let repeated_vertex = face[0] == face[1] || face[1] == face[2] || face[0] == face[2];
            let area = triangle_area_from_vertices(&mesh.vertices, *face);
            if !repeated_vertex && area > f32::EPSILON {
                return None;
            }

            let mut metrics = HashMap::new();
            metrics.insert("area_mm2".to_string(), area as f64);
            metrics.insert(
                "repeated_vertex".to_string(),
                if repeated_vertex { 1.0 } else { 0.0 },
            );
            Some(Finding {
                rule_id: "mesh/degenerate-face".to_string(),
                severity: Severity::Error,
                message: format!("face {face_id} is degenerate"),
                face_ids: vec![face_id as u32],
                metrics,
                recommendation: Some("Run with --fix to remove degenerate faces.".into()),
            })
        })
        .collect()
}

fn check_duplicate_faces(mesh: &Mesh) -> Vec<Finding> {
    let mut seen = HashMap::<[u32; 3], u32>::new();
    let mut findings = Vec::new();

    for (face_id, face) in mesh.faces.iter().enumerate() {
        let mut sorted = *face;
        sorted.sort_unstable();
        if let Some(original_face_id) = seen.get(&sorted).copied() {
            let mut metrics = HashMap::new();
            metrics.insert("original_face".to_string(), original_face_id as f64);
            metrics.insert("duplicate_face".to_string(), face_id as f64);
            findings.push(Finding {
                rule_id: "mesh/duplicate-face".to_string(),
                severity: Severity::Warn,
                message: format!("face {face_id} duplicates face {original_face_id}"),
                face_ids: vec![original_face_id, face_id as u32],
                metrics,
                recommendation: Some("Run with --fix to remove duplicate faces.".into()),
            });
        } else {
            seen.insert(sorted, face_id as u32);
        }
    }

    findings
}

fn check_non_manifold(mesh: &Mesh) -> Vec<Finding> {
    let mut edge_faces: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (face_id, face) in mesh.faces.iter().enumerate() {
        for edge in face_edges(*face) {
            edge_faces
                .entry(canonical_edge(edge))
                .or_default()
                .push(face_id as u32);
        }
    }

    let mut bad_edges = edge_faces
        .into_iter()
        .filter(|(_, face_ids)| face_ids.len() != 2)
        .collect::<Vec<_>>();
    bad_edges.sort_by_key(|((a, b), _)| (*a, *b));

    bad_edges
        .into_iter()
        .map(|((a, b), face_ids)| {
            let mut metrics = HashMap::new();
            metrics.insert("vertex_a".to_string(), a as f64);
            metrics.insert("vertex_b".to_string(), b as f64);
            metrics.insert("faces".to_string(), face_ids.len() as f64);
            Finding {
                rule_id: "mesh/non-manifold".to_string(),
                severity: Severity::Error,
                message: format!(
                    "edge {a}-{b} is shared by {} faces; expected 2",
                    face_ids.len()
                ),
                face_ids,
                metrics,
                recommendation: Some(
                    "Repair open boundaries, duplicate surfaces, or T-junctions before printing."
                        .into(),
                ),
            }
        })
        .collect()
}

fn check_boundary_loops(mesh: &Mesh) -> Vec<Finding> {
    let boundary_edges = collect_boundary_edges(mesh);
    if boundary_edges.is_empty() {
        return Vec::new();
    }

    let mut edges_by_vertex: HashMap<u32, Vec<usize>> = HashMap::new();
    for (edge_id, boundary_edge) in boundary_edges.iter().enumerate() {
        edges_by_vertex
            .entry(boundary_edge.edge.0)
            .or_default()
            .push(edge_id);
        edges_by_vertex
            .entry(boundary_edge.edge.1)
            .or_default()
            .push(edge_id);
    }

    let mut visited = vec![false; boundary_edges.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for start in 0..boundary_edges.len() {
        if visited[start] {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;

        while let Some(edge_id) = queue.pop_front() {
            component.push(edge_id);
            let edge = boundary_edges[edge_id].edge;
            for vertex_id in [edge.0, edge.1] {
                if let Some(neighbor_edges) = edges_by_vertex.get(&vertex_id) {
                    for &neighbor_edge in neighbor_edges {
                        if !visited[neighbor_edge] {
                            visited[neighbor_edge] = true;
                            queue.push_back(neighbor_edge);
                        }
                    }
                }
            }
        }

        components.push(component);
    }

    components
        .into_iter()
        .enumerate()
        .map(|(component_id, component)| {
            let mut face_ids = component
                .iter()
                .map(|&edge_id| boundary_edges[edge_id].face_id)
                .collect::<Vec<_>>();
            face_ids.sort_unstable();
            face_ids.dedup();

            let mut vertex_degrees = HashMap::<u32, usize>::new();
            for &edge_id in &component {
                let edge = boundary_edges[edge_id].edge;
                *vertex_degrees.entry(edge.0).or_default() += 1;
                *vertex_degrees.entry(edge.1).or_default() += 1;
            }

            let closed = vertex_degrees.values().all(|degree| *degree == 2);
            let mut metrics = HashMap::new();
            metrics.insert("component".to_string(), component_id as f64);
            metrics.insert("edges".to_string(), component.len() as f64);
            metrics.insert("vertices".to_string(), vertex_degrees.len() as f64);
            metrics.insert("closed".to_string(), if closed { 1.0 } else { 0.0 });

            let message = if closed {
                format!(
                    "boundary loop {component_id} has {} edges; mesh surface is open",
                    component.len()
                )
            } else {
                format!(
                    "open boundary component {component_id} has {} edges and {} vertices",
                    component.len(),
                    vertex_degrees.len()
                )
            };

            Finding {
                rule_id: "mesh/boundary-loop".to_string(),
                severity: Severity::Error,
                message,
                face_ids,
                metrics,
                recommendation: Some(
                    "Fill the boundary, reconnect nearby vertices, or repair the source surface."
                        .into(),
                ),
            }
        })
        .collect()
}

fn check_normal_consistency(mesh: &Mesh) -> Vec<Finding> {
    let mut edge_faces: HashMap<Edge, Vec<FaceEdge>> = HashMap::new();
    for (face_id, face) in mesh.faces.iter().enumerate() {
        for edge in face_edges(*face) {
            edge_faces
                .entry(canonical_edge(edge))
                .or_default()
                .push((face_id as u32, edge));
        }
    }

    let mut findings = Vec::new();
    let mut edge_faces = edge_faces.into_iter().collect::<Vec<_>>();
    edge_faces.sort_by_key(|((a, b), _)| (*a, *b));

    for ((a, b), faces) in edge_faces {
        if faces.len() != 2 {
            continue;
        }
        let (face_a, edge_a) = faces[0];
        let (face_b, edge_b) = faces[1];
        if edge_a == edge_b {
            let mut metrics = HashMap::new();
            metrics.insert("vertex_a".to_string(), a as f64);
            metrics.insert("vertex_b".to_string(), b as f64);
            findings.push(Finding {
                rule_id: "mesh/inconsistent-normals".to_string(),
                severity: Severity::Warn,
                message: format!(
                    "faces {face_a} and {face_b} traverse shared edge {a}-{b} in the same direction"
                ),
                face_ids: vec![face_a, face_b],
                metrics,
                recommendation: Some(
                    "Run with --fix to reorient face winding consistently.".into(),
                ),
            });
        }
    }
    findings
}

fn check_shells(mesh: &Mesh, tiny_volume: f32) -> Vec<Finding> {
    let shells = connected_shells(mesh);
    let mut findings = Vec::new();
    let primary_shell = primary_shell_index(mesh, &shells);

    for (index, shell) in shells.iter().enumerate() {
        if Some(index) == primary_shell {
            continue;
        }

        let shell_size = shell_size_estimate(mesh, shell);
        let mut metrics = HashMap::new();
        metrics.insert("shell".to_string(), index as f64);
        metrics.insert("faces".to_string(), shell.len() as f64);
        metrics.insert("size_estimate_mm3".to_string(), shell_size as f64);
        metrics.insert("threshold_mm3".to_string(), tiny_volume as f64);

        let face_ids = shell.iter().map(|face_id| *face_id as u32).collect();
        if shell_size < tiny_volume {
            findings.push(Finding {
                rule_id: "mesh/tiny-shell".to_string(),
                severity: Severity::Warn,
                message: format!(
                    "shell {index} has {} faces and is below {tiny_volume:.3} mm3",
                    shell.len()
                ),
                face_ids,
                metrics,
                recommendation: Some("Run with --fix to remove tiny loose shells.".into()),
            });
        } else {
            findings.push(Finding {
                rule_id: "mesh/disconnected-shell".to_string(),
                severity: Severity::Warn,
                message: format!(
                    "shell {index} is disconnected from the primary shell and has {} faces",
                    shell.len()
                ),
                face_ids,
                metrics,
                recommendation: Some("Check whether this loose shell is intentional.".into()),
            });
        }
    }

    if shells.len() > 1 {
        let mut metrics = HashMap::new();
        metrics.insert("shells".to_string(), shells.len() as f64);
        findings.push(Finding {
            rule_id: "mesh/disconnected-shells-summary".to_string(),
            severity: Severity::Info,
            message: format!("mesh contains {} disconnected shells", shells.len()),
            face_ids: Vec::new(),
            metrics,
            recommendation: None,
        });
    }

    findings
}

fn check_inverted_normals(mesh: &Mesh) -> Vec<Finding> {
    let volume = signed_volume(mesh);
    if volume >= 0.0 {
        return Vec::new();
    }

    let mut metrics = HashMap::new();
    metrics.insert("signed_volume_mm3".to_string(), volume as f64);
    vec![Finding {
        rule_id: "mesh/inverted-normals".to_string(),
        severity: Severity::Warn,
        message: "mesh winding appears globally inverted".to_string(),
        face_ids: Vec::new(),
        metrics,
        recommendation: Some("Run with --fix to flip face winding outward.".into()),
    }]
}

fn check_self_intersections(mesh: &Mesh) -> Vec<Finding> {
    let face_boxes = mesh
        .faces
        .iter()
        .map(|face| face_bounds(mesh, *face))
        .collect::<Vec<_>>();
    let mut face_order = (0..mesh.faces.len()).collect::<Vec<_>>();
    face_order.sort_by(|a, b| {
        face_boxes[*a].0[0]
            .partial_cmp(&face_boxes[*b].0[0])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut findings = Vec::new();

    for (order_index, &a_id) in face_order.iter().enumerate() {
        let a_max_x = face_boxes[a_id].1[0];
        for &b_id in &face_order[order_index + 1..] {
            if face_boxes[b_id].0[0] > a_max_x + 1.0e-6 {
                break;
            }
            let face_a = mesh.faces[a_id];
            let face_b = mesh.faces[b_id];
            if faces_share_vertex(face_a, face_b)
                || !bounds_overlap(face_boxes[a_id], face_boxes[b_id])
            {
                continue;
            }

            if triangles_intersect(mesh, face_a, face_b) {
                let mut metrics = HashMap::new();
                metrics.insert("face_a".to_string(), a_id as f64);
                metrics.insert("face_b".to_string(), b_id as f64);
                findings.push(Finding {
                    rule_id: "mesh/self-intersection".to_string(),
                    severity: Severity::Error,
                    message: format!("faces {a_id} and {b_id} intersect"),
                    face_ids: vec![a_id as u32, b_id as u32],
                    metrics,
                    recommendation: Some(
                        "Separate or boolean-repair intersecting surfaces in the source model."
                            .into(),
                    ),
                });
            }
        }
    }

    findings
}

fn check_large_cross_sections(mesh: &Mesh, thresholds: &Thresholds) -> Vec<Finding> {
    if thresholds.layer_height_mm <= 0.0 || thresholds.large_cross_section_mm2 <= 0.0 {
        return Vec::new();
    }

    let mut layers = HashMap::<i64, (f32, Vec<u32>)>::new();
    for (face_id, face) in mesh.faces.iter().enumerate() {
        let [a, b, c] = face_vertices(mesh, *face);
        let area = projected_xy_area(a, b, c);
        if area <= f32::EPSILON {
            continue;
        }

        let z = (a[2] + b[2] + c[2]) / 3.0;
        let layer = (z / thresholds.layer_height_mm).round() as i64;
        let entry = layers.entry(layer).or_default();
        entry.0 += area;
        entry.1.push(face_id as u32);
    }

    let mut layers = layers.into_iter().collect::<Vec<_>>();
    layers.sort_by_key(|(layer, _)| *layer);

    layers
        .into_iter()
        .filter_map(|(layer, (area, mut face_ids))| {
            if area < thresholds.large_cross_section_mm2 {
                return None;
            }
            face_ids.sort_unstable();
            face_ids.dedup();

            let mut metrics = HashMap::new();
            metrics.insert("layer".to_string(), layer as f64);
            metrics.insert("z_mm".to_string(), layer as f64 * thresholds.layer_height_mm as f64);
            metrics.insert("area_mm2".to_string(), area as f64);
            metrics.insert(
                "threshold_mm2".to_string(),
                thresholds.large_cross_section_mm2 as f64,
            );

            Some(Finding {
                rule_id: "sla/large-cross-section".to_string(),
                severity: Severity::Warn,
                message: format!(
                    "large cross-section near layer {layer} is {area:.2} mm2; threshold is {:.2} mm2",
                    thresholds.large_cross_section_mm2
                ),
                face_ids,
                metrics,
                recommendation: Some(
                    "Reorient the model or reduce large flat peel areas for SLA printing.".into(),
                ),
            })
        })
        .collect()
}

fn weld_vertices(mesh: &mut Mesh, tolerance: f32) -> Vec<Fix> {
    if tolerance <= 0.0 || mesh.vertices.is_empty() {
        return Vec::new();
    }

    let before = mesh.vertices.len();
    let tolerance_sq = tolerance * tolerance;
    let scale = 1.0 / tolerance;
    let mut remap = vec![0u32; mesh.vertices.len()];
    let mut unique = Vec::<[f32; 3]>::new();
    let mut buckets = HashMap::<(i64, i64, i64), Vec<u32>>::new();

    for (id, vertex) in mesh.vertices.iter().enumerate() {
        let key = (
            (vertex[0] * scale).floor() as i64,
            (vertex[1] * scale).floor() as i64,
            (vertex[2] * scale).floor() as i64,
        );
        let mut new_id = None;
        'search: for x in key.0 - 1..=key.0 + 1 {
            for y in key.1 - 1..=key.1 + 1 {
                for z in key.2 - 1..=key.2 + 1 {
                    if let Some(candidates) = buckets.get(&(x, y, z)) {
                        for &candidate in candidates {
                            if distance_sq(*vertex, unique[candidate as usize]) <= tolerance_sq {
                                new_id = Some(candidate);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        let new_id = new_id.unwrap_or_else(|| {
            let new_id = unique.len() as u32;
            unique.push(*vertex);
            buckets.entry(key).or_default().push(new_id);
            new_id
        });
        remap[id] = new_id;
    }

    for face in &mut mesh.faces {
        for vertex_id in face {
            *vertex_id = remap[*vertex_id as usize];
        }
    }
    mesh.vertices = unique;

    let removed = before - mesh.vertices.len();
    if removed == 0 {
        return Vec::new();
    }

    vec![fix_with_count(
        "fix/weld-vertices",
        format!("merged {removed} vertices within {tolerance:.4} mm"),
        "vertices",
        removed,
    )]
}

fn remove_degenerate_faces(mesh: &mut Mesh) -> Vec<Fix> {
    let before = mesh.faces.len();
    let vertices = &mesh.vertices;
    mesh.faces.retain(|face| {
        face[0] != face[1]
            && face[1] != face[2]
            && face[0] != face[2]
            && triangle_area_from_vertices(vertices, *face) > 0.0
    });
    let removed = before - mesh.faces.len();
    if removed == 0 {
        Vec::new()
    } else {
        vec![fix_with_count(
            "fix/remove-degenerate-faces",
            format!("removed {removed} degenerate faces"),
            "faces",
            removed,
        )]
    }
}

fn remove_duplicate_faces(mesh: &mut Mesh) -> Vec<Fix> {
    let before = mesh.faces.len();
    let mut seen = HashSet::new();
    mesh.faces.retain(|face| {
        let mut sorted = *face;
        sorted.sort_unstable();
        seen.insert(sorted)
    });
    let removed = before - mesh.faces.len();
    if removed == 0 {
        Vec::new()
    } else {
        vec![fix_with_count(
            "fix/remove-duplicate-faces",
            format!("removed {removed} duplicate faces"),
            "faces",
            removed,
        )]
    }
}

fn orient_faces_consistently(mesh: &mut Mesh) -> Vec<Fix> {
    let adjacency = face_adjacency(mesh);
    let mut visited = vec![false; mesh.faces.len()];
    let mut flipped = 0usize;

    for start in 0..mesh.faces.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(face_id) = queue.pop_front() {
            for &neighbor_id in &adjacency[face_id] {
                if visited[neighbor_id] {
                    continue;
                }
                if shared_edge_same_direction(mesh.faces[face_id], mesh.faces[neighbor_id]) {
                    mesh.faces[neighbor_id].swap(1, 2);
                    flipped += 1;
                }
                visited[neighbor_id] = true;
                queue.push_back(neighbor_id);
            }
        }
    }

    if signed_volume(mesh) < 0.0 {
        for face in &mut mesh.faces {
            face.swap(1, 2);
        }
        flipped += mesh.faces.len();
    }

    if flipped == 0 {
        Vec::new()
    } else {
        vec![fix_with_count(
            "fix/orient-faces",
            format!("rewound {flipped} faces for consistent normals"),
            "faces",
            flipped,
        )]
    }
}

fn remove_tiny_shells(mesh: &mut Mesh, threshold: f32) -> Vec<Fix> {
    let shells = connected_shells(mesh);
    let primary_shell = primary_shell_index(mesh, &shells);
    let keep_faces: HashSet<usize> = shells
        .iter()
        .enumerate()
        .filter(|(index, shell)| {
            Some(*index) == primary_shell || shell_size_estimate(mesh, shell) >= threshold
        })
        .flat_map(|(_, shell)| shell.iter().copied())
        .collect();
    let before = mesh.faces.len();
    mesh.faces = mesh
        .faces
        .iter()
        .enumerate()
        .filter_map(|(id, face)| keep_faces.contains(&id).then_some(*face))
        .collect();
    remove_unreferenced_vertices(mesh);

    let removed = before - mesh.faces.len();
    if removed == 0 {
        Vec::new()
    } else {
        vec![fix_with_count(
            "fix/remove-tiny-shells",
            format!("removed {removed} faces from tiny disconnected shells"),
            "faces",
            removed,
        )]
    }
}

fn fix_with_count(rule_id: &str, message: String, key: &str, count: usize) -> Fix {
    let mut metrics = HashMap::new();
    metrics.insert(key.to_string(), count as f64);
    Fix {
        rule_id: rule_id.to_string(),
        message,
        metrics,
    }
}

fn connected_shells(mesh: &Mesh) -> Vec<Vec<usize>> {
    let adjacency = face_adjacency(mesh);
    let mut visited = vec![false; mesh.faces.len()];
    let mut shells = Vec::new();

    for start in 0..mesh.faces.len() {
        if visited[start] {
            continue;
        }
        let mut shell = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(face_id) = queue.pop_front() {
            shell.push(face_id);
            for &neighbor in &adjacency[face_id] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        shells.push(shell);
    }

    shells
}

fn collect_boundary_edges(mesh: &Mesh) -> Vec<BoundaryEdge> {
    let mut edge_faces: HashMap<Edge, Vec<u32>> = HashMap::new();
    for (face_id, face) in mesh.faces.iter().enumerate() {
        for edge in face_edges(*face) {
            edge_faces
                .entry(canonical_edge(edge))
                .or_default()
                .push(face_id as u32);
        }
    }

    let mut boundary_edges = edge_faces
        .into_iter()
        .filter_map(|(edge, face_ids)| {
            (face_ids.len() == 1).then_some(BoundaryEdge {
                edge,
                face_id: face_ids[0],
            })
        })
        .collect::<Vec<_>>();
    boundary_edges.sort_by_key(|boundary_edge| boundary_edge.edge);
    boundary_edges
}

fn primary_shell_index(mesh: &Mesh, shells: &[Vec<usize>]) -> Option<usize> {
    shells
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            shell_size_estimate(mesh, a)
                .partial_cmp(&shell_size_estimate(mesh, b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.len().cmp(&b.len()))
        })
        .map(|(index, _)| index)
}

fn face_adjacency(mesh: &Mesh) -> Vec<Vec<usize>> {
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (face_id, face) in mesh.faces.iter().enumerate() {
        for edge in face_edges(*face) {
            edge_faces
                .entry(canonical_edge(edge))
                .or_default()
                .push(face_id);
        }
    }

    let mut adjacency = vec![Vec::new(); mesh.faces.len()];
    for faces in edge_faces.values() {
        for &a in faces {
            for &b in faces {
                if a != b {
                    adjacency[a].push(b);
                }
            }
        }
    }
    adjacency
}

fn remove_unreferenced_vertices(mesh: &mut Mesh) {
    let mut remap = vec![None; mesh.vertices.len()];
    let mut vertices = Vec::new();
    for face in &mesh.faces {
        for &vertex_id in face {
            let slot = &mut remap[vertex_id as usize];
            if slot.is_none() {
                *slot = Some(vertices.len() as u32);
                vertices.push(mesh.vertices[vertex_id as usize]);
            }
        }
    }
    for face in &mut mesh.faces {
        for vertex_id in face {
            *vertex_id = remap[*vertex_id as usize].unwrap();
        }
    }
    mesh.vertices = vertices;
}

fn shell_size_estimate(mesh: &Mesh, shell: &[usize]) -> f32 {
    let volume = shell_signed_volume(mesh, shell).abs();
    if volume > f32::EPSILON {
        volume
    } else {
        shell_bounding_box_volume(mesh, shell)
    }
}

fn shell_bounding_box_volume(mesh: &Mesh, shell: &[usize]) -> f32 {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for &face_id in shell {
        for vertex_id in mesh.faces[face_id] {
            let vertex = mesh.vertices[vertex_id as usize];
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex[axis]);
                max[axis] = max[axis].max(vertex[axis]);
            }
        }
    }
    (max[0] - min[0]).max(0.0) * (max[1] - min[1]).max(0.0) * (max[2] - min[2]).max(0.0)
}

fn shell_signed_volume(mesh: &Mesh, shell: &[usize]) -> f32 {
    shell
        .iter()
        .map(|&face_id| {
            let face = mesh.faces[face_id];
            let a = mesh.vertices[face[0] as usize];
            let b = mesh.vertices[face[1] as usize];
            let c = mesh.vertices[face[2] as usize];
            dot(a, cross(b, c)) / 6.0
        })
        .sum()
}

fn face_edges(face: [u32; 3]) -> [(u32, u32); 3] {
    [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])]
}

fn canonical_edge(edge: (u32, u32)) -> (u32, u32) {
    if edge.0 < edge.1 {
        edge
    } else {
        (edge.1, edge.0)
    }
}

fn shared_edge_same_direction(a: [u32; 3], b: [u32; 3]) -> bool {
    face_edges(a).iter().any(|edge_a| {
        face_edges(b)
            .iter()
            .any(|edge_b| canonical_edge(*edge_a) == canonical_edge(*edge_b) && edge_a == edge_b)
    })
}

fn face_normal(mesh: &Mesh, face: [u32; 3]) -> [f32; 3] {
    let [a, b, c] = face_vertices(mesh, face);
    let normal = cross(sub(b, a), sub(c, a));
    let length = dot(normal, normal).sqrt();
    if length == 0.0 {
        [0.0, 0.0, 0.0]
    } else {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    }
}

fn face_vertices(mesh: &Mesh, face: [u32; 3]) -> [[f32; 3]; 3] {
    [
        mesh.vertices[face[0] as usize],
        mesh.vertices[face[1] as usize],
        mesh.vertices[face[2] as usize],
    ]
}

fn face_bounds(mesh: &Mesh, face: [u32; 3]) -> ([f32; 3], [f32; 3]) {
    let vertices = face_vertices(mesh, face);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    (min, max)
}

fn bounds_overlap(a: ([f32; 3], [f32; 3]), b: ([f32; 3], [f32; 3])) -> bool {
    const EPS: f32 = 1.0e-6;
    (0..3).all(|axis| a.0[axis] <= b.1[axis] + EPS && b.0[axis] <= a.1[axis] + EPS)
}

fn faces_share_vertex(a: [u32; 3], b: [u32; 3]) -> bool {
    a.iter().any(|a_vertex| b.contains(a_vertex))
}

fn triangles_intersect(mesh: &Mesh, face_a: [u32; 3], face_b: [u32; 3]) -> bool {
    let a = face_vertices(mesh, face_a);
    let b = face_vertices(mesh, face_b);

    triangle_edges(a)
        .iter()
        .any(|(start, end)| segment_intersects_triangle(*start, *end, b))
        || triangle_edges(b)
            .iter()
            .any(|(start, end)| segment_intersects_triangle(*start, *end, a))
}

fn triangle_edges(vertices: [[f32; 3]; 3]) -> [([f32; 3], [f32; 3]); 3] {
    [
        (vertices[0], vertices[1]),
        (vertices[1], vertices[2]),
        (vertices[2], vertices[0]),
    ]
}

fn segment_intersects_triangle(start: [f32; 3], end: [f32; 3], tri: [[f32; 3]; 3]) -> bool {
    const EPS: f32 = 1.0e-6;
    let direction = sub(end, start);
    let edge1 = sub(tri[1], tri[0]);
    let edge2 = sub(tri[2], tri[0]);
    let h = cross(direction, edge2);
    let det = dot(edge1, h);

    if det.abs() < EPS {
        return false;
    }

    let inv_det = 1.0 / det;
    let s = sub(start, tri[0]);
    let u = inv_det * dot(s, h);
    if !(EPS..=1.0 - EPS).contains(&u) {
        return false;
    }

    let q = cross(s, edge1);
    let v = inv_det * dot(direction, q);
    if v < EPS || u + v > 1.0 - EPS {
        return false;
    }

    let t = inv_det * dot(edge2, q);
    (EPS..=1.0 - EPS).contains(&t)
}

fn triangle_area_from_vertices(vertices: &[[f32; 3]], face: [u32; 3]) -> f32 {
    let a = vertices[face[0] as usize];
    let b = vertices[face[1] as usize];
    let c = vertices[face[2] as usize];
    dot(cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(c, a))).sqrt() * 0.5
}

fn projected_xy_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
}

fn signed_volume(mesh: &Mesh) -> f32 {
    mesh.faces
        .iter()
        .map(|face| {
            let a = mesh.vertices[face[0] as usize];
            let b = mesh.vertices[face[1] as usize];
            let c = mesh.vertices[face[2] as usize];
            dot(a, cross(b, c)) / 6.0
        })
        .sum()
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let delta = sub(a, b);
    dot(delta, delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ascii_stl() {
        let bytes = b"solid t
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
endsolid t";
        let mesh = parse_stl(bytes).unwrap();
        assert_eq!(mesh.faces.len(), 1);
        assert_eq!(mesh.vertices.len(), 3);
    }

    #[test]
    fn removes_duplicate_vertices_and_degenerate_faces() {
        let mut mesh = Mesh {
            vertices: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            faces: vec![[0, 1, 2]],
        };
        let fixes = fix_mesh(&mut mesh, &LintOptions::default());
        assert!(fixes.iter().any(|fix| fix.rule_id == "fix/weld-vertices"));
        assert!(mesh.faces.is_empty());
    }
}

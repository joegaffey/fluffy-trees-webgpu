struct Globals {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  camera_pos: vec4<f32>,
  sun_dir: vec4<f32>,
  cam_right: vec4<f32>,
  cam_up: vec4<f32>,
  cam_fwd: vec4<f32>,
  time: vec4<f32>,
  fog_color: vec4<f32>,
  misc: vec4<f32>,
  style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> G: Globals;

const PI: f32 = 3.14159265;

fn hash21(p: vec2<f32>) -> f32 {
  var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
  p3 = p3 + dot(p3, vec3<f32>(p3.y, p3.z, p3.x) + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = hash21(i);
  let b = hash21(i + vec2<f32>(1.0, 0.0));
  let c = hash21(i + vec2<f32>(0.0, 1.0));
  let d = hash21(i + vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
  var s = 0.0;
  var a = 0.5;
  var q = p;
  for (var i = 0; i < 4; i = i + 1) {
    s = s + a * vnoise(q);
    q = q * 2.03 + vec2<f32>(13.7, 7.3);
    a = a * 0.5;
  }
  return s;
}

fn fog_amount(dist: f32) -> f32 {
  let x = dist * G.fog_color.w;
  return clamp(1.0 - exp(-x * x), 0.0, 1.0);
}

// ---------------------------------------------------------------- sky

struct SkyOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
  var p = vec2<f32>(-1.0, -1.0);
  if (vi == 1u) { p = vec2<f32>(3.0, -1.0); }
  if (vi == 2u) { p = vec2<f32>(-1.0, 3.0); }
  var out: SkyOut;
  out.clip = vec4<f32>(p, 1.0, 1.0);
  out.ndc = p;
  return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
  let tan_y = G.misc.x;
  let aspect = G.misc.y;
  let dir = normalize(
    G.cam_fwd.xyz
      + G.cam_right.xyz * (in.ndc.x * tan_y * aspect)
      + G.cam_up.xyz * (in.ndc.y * tan_y)
  );
  let up = clamp(dir.y, 0.0, 1.0);
  var col = mix(vec3<f32>(0.62, 0.79, 0.98), vec3<f32>(0.10, 0.32, 0.78), pow(up, 0.55));
  let horizon = vec3<f32>(0.88, 0.82, 0.72);
  col = mix(horizon, col, smoothstep(-0.02, 0.26, dir.y));
  let sun = max(dot(dir, G.sun_dir.xyz), 0.0);
  col = col + vec3<f32>(1.0, 0.74, 0.38) * pow(sun, 1200.0) * 2.4;
  col = col + vec3<f32>(1.0, 0.64, 0.30) * pow(sun, 12.0) * 0.08;
  return vec4<f32>(col, 1.0);
}

// ---------------------------------------------------------------- ground

struct GroundIn {
  @location(0) local: vec2<f32>,
};

struct GroundOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
};

@vertex
fn vs_ground(in: GroundIn) -> GroundOut {
  let world = vec3<f32>(in.local.x * 2600.0, 0.0, G.time.z + in.local.y * 2600.0);
  var out: GroundOut;
  out.clip = G.view_proj * vec4<f32>(world, 1.0);
  out.world = world;
  return out;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
  let w = in.world;
  let dist = length(w - G.camera_pos.xyz);
  let near = smoothstep(260.0, 45.0, dist);
  let detail = vnoise(w.xz * 0.7);
  let broad = fbm(w.xz * 0.075);

  var grass = mix(vec3<f32>(0.085, 0.185, 0.042), vec3<f32>(0.26, 0.46, 0.11), broad);
  grass = grass * (0.74 + 0.48 * detail * mix(0.4, 1.0, near));
  let dry = smoothstep(0.55, 0.95, fbm(w.xz * 0.03 + vec2<f32>(11.0, 3.0)));
  grass = mix(grass, vec3<f32>(0.36, 0.39, 0.14), dry * 0.5);

  var road = vec3<f32>(0.125, 0.13, 0.14) * (0.72 + 0.5 * vnoise(w.xz * 1.9));
  road = road * (0.9 + 0.1 * smoothstep(4.0, 8.0, abs(w.x)));
  let dash = step(0.5, fract(w.z * 0.105));
  let centerline = smoothstep(0.22, 0.11, abs(w.x)) * dash * near;
  road = mix(road, vec3<f32>(0.80, 0.76, 0.54), centerline * 0.9);
  let edge = smoothstep(0.20, 0.06, abs(abs(w.x) - 7.9)) * near;
  road = mix(road, vec3<f32>(0.66, 0.66, 0.61), edge * 0.8);

  let road_mask = smoothstep(8.7, 7.5, abs(w.x));
  var col = mix(grass, road, road_mask);
  let forest_floor = smoothstep(10.0, 26.0, abs(w.x));
  col = col * (1.0 - 0.16 * forest_floor);
  col = col * (0.92 + 0.08 * mix(0.6, 1.0, near));

  col = mix(col, G.fog_color.rgb, fog_amount(dist));
  return vec4<f32>(col, 1.0);
}

// ---------------------------------------------------------------- trunk

struct TrunkIn {
  @location(0) pos: vec3<f32>,
  @location(1) nrm: vec3<f32>,
  @location(2) inst_pos_scale: vec4<f32>,
  @location(3) inst_params: vec4<f32>,
};

struct TrunkOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) normal: vec3<f32>,
  @location(1) world: vec3<f32>,
  @location(2) tint: f32,
  @location(3) local_y: f32,
};

@vertex
fn vs_trunk(in: TrunkIn) -> TrunkOut {
  let s = in.inst_pos_scale.w;
  let yaw = in.inst_params.x;
  let ca = cos(yaw);
  let sa = sin(yaw);

  let p = in.pos * s;
  let rp = vec3<f32>(ca * p.x - sa * p.z, p.y, sa * p.x + ca * p.z);
  let n = in.nrm;
  let rn = vec3<f32>(ca * n.x - sa * n.z, n.y, sa * n.x + ca * n.z);

  let rel = in.inst_pos_scale.z - G.time.z;
  let wrapped = rel - G.misc.z * floor(rel / G.misc.z + 0.5);
  let base = vec3<f32>(in.inst_pos_scale.x, in.inst_pos_scale.y, G.time.z + wrapped);
  let world = base + rp;

  var out: TrunkOut;
  out.clip = G.view_proj * vec4<f32>(world, 1.0);
  out.normal = rn;
  out.world = world;
  out.tint = in.inst_params.y;
  out.local_y = in.pos.y;
  return out;
}

@fragment
fn fs_trunk(in: TrunkOut) -> @location(0) vec4<f32> {
  let n = normalize(in.normal);
  let wrap = dot(n, G.sun_dir.xyz) * 0.5 + 0.5;
  var base = mix(vec3<f32>(0.13, 0.09, 0.05), vec3<f32>(0.35, 0.25, 0.145), in.tint);
  base = base * (0.88 + 0.35 * vnoise(in.world.xz * 9.0 + vec2<f32>(in.local_y * 6.0, 0.0)));
  base = mix(base, base * 1.2, smoothstep(2.5, 4.8, in.local_y));
  var col = base * (0.55 + 0.55 * wrap);
  let dist = length(in.world - G.camera_pos.xyz);
  col = mix(col, G.fog_color.rgb, fog_amount(dist));
  return vec4<f32>(col, 1.0);
}

// ---------------------------------------------------------------- foliage cards

struct CardIn {
  @location(0) tree_pos: vec4<f32>,
  @location(1) offset: vec4<f32>,
  @location(2) extra: vec4<f32>,
  @location(3) normal_shade: vec4<f32>,
};

struct CardOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) tint: f32,
  @location(2) kind: f32,
  @location(3) world: vec3<f32>,
  @location(4) normal: vec3<f32>,
  @location(5) shade: f32,
};

@vertex
fn vs_card(@builtin(vertex_index) vi: u32, inst: CardIn) -> CardOut {
  var c = vec2<f32>(-1.0, -1.0);
  if (vi == 1u || vi == 3u || vi == 4u) { c.x = 1.0; }
  if (vi == 2u || vi == 4u || vi == 5u) { c.y = 1.0; }

  let rel = inst.tree_pos.z - G.time.z;
  let wrapped = rel - G.misc.z * floor(rel / G.misc.z + 0.5);
  let tree_center = vec3<f32>(inst.tree_pos.x, inst.tree_pos.y, G.time.z + wrapped);

  let local = inst.offset.xyz;
  let sway_phase = inst.tree_pos.w + local.y * 0.7;
  let wind = G.time.y;
  let sway = (sin(G.time.x * 1.25 + sway_phase) + 0.35 * sin(G.time.x * 2.7 + sway_phase * 1.7))
    * wind * (0.12 + local.y * 0.05);
  let center = tree_center + vec3<f32>(local.x + sway, local.y + sway * 0.25, local.z + sway * 0.7);

  let ca = cos(inst.extra.y);
  let sa = sin(inst.extra.y);
  let rc = vec2<f32>(ca * c.x - sa * c.y, sa * c.x + ca * c.y);

  let n = normalize(inst.normal_shade.xyz);
  var up_axis = vec3<f32>(0.0, 1.0, 0.0);
  if (abs(n.y) > 0.92) { up_axis = vec3<f32>(1.0, 0.0, 0.0); }
  let tang = normalize(cross(n, up_axis));
  let bita = normalize(cross(n, tang));

  let local_off = tang * (rc.x * inst.offset.w) + bita * (rc.y * inst.extra.x);
  let camera_off = G.cam_right.xyz * (rc.x * inst.offset.w) + vec3<f32>(0.0, 1.0, 0.0) * (rc.y * inst.extra.x);
  let billboard = clamp(G.style.y, 0.0, 1.0);
  let wp = center + mix(local_off, camera_off, billboard);

  var out: CardOut;
  out.clip = G.view_proj * vec4<f32>(wp, 1.0);
  out.uv = c;
  out.tint = inst.extra.z;
  out.kind = inst.extra.w;
  out.world = wp;
  out.normal = n;
  out.shade = inst.normal_shade.w;
  return out;
}

@fragment
fn fs_card(in: CardOut) -> @location(0) vec4<f32> {
  let p = in.uv;
  let d = length(p);
  let seed = in.kind * 13.7 + in.tint * 7.1;

  let n1 = fbm(p * 2.1 + vec2<f32>(seed, seed * 0.73));
  let n2 = fbm(p * 5.3 - vec2<f32>(seed * 1.31, seed * 0.57));

  var edge = d;
  edge = edge + (n1 - 0.5) * 0.52;
  edge = edge + 0.06 * sin(p.x * 7.0 + seed * 2.0);
  edge = edge + 0.06 * cos(p.y * 9.0 - seed * 3.0);
  edge = edge + 0.09 * sin(p.x * 4.0 + seed) * cos(p.y * 5.0 - seed * 1.7);

  var alpha = 1.0 - smoothstep(0.58, 0.90, edge);
  alpha = alpha * (1.0 - smoothstep(0.92, 1.02, d));
  alpha = alpha * smoothstep(0.18, 0.42, 0.30 + n2 * 0.75);

  let n = normalize(in.normal);
  let view_dir = normalize(G.camera_pos.xyz - in.world);
  let wrap = dot(n, G.sun_dir.xyz) * 0.5 + 0.5;
  let painterly = smoothstep(0.10, 0.88, wrap);
  let cel = floor(wrap * 3.0) / 3.0;
  let band = mix(painterly, cel, clamp(G.style.x, 0.0, 1.0));

  var base = mix(vec3<f32>(0.09, 0.23, 0.075), vec3<f32>(0.20, 0.39, 0.105), in.tint);
  base = mix(base, vec3<f32>(0.27, 0.44, 0.13), smoothstep(0.45, 1.0, in.tint) * 0.35);

  let clump = 0.90 + 0.20 * n2;
  var col = base * ((0.52 + 0.52 * band) * clump) * in.shade;
  let fres = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0);
  col = col + vec3<f32>(0.50, 0.66, 0.28) * fres * mix(0.14, 0.34, G.style.x) * band;

  let dist = length(in.world - G.camera_pos.xyz);
  col = mix(col, G.fog_color.rgb, fog_amount(dist));
  alpha = alpha * (1.0 - smoothstep(330.0, 420.0, dist));
  return vec4<f32>(col, alpha);
}

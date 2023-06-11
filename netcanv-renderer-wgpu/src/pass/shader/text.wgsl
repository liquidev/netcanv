const max_glyph_count = 1024u;
const max_atlas_glyph_count = 1024u;

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) glyph_index: u32,
   @location(1) uv: vec2f,
}

struct Glyph {
   @align(16)
   position: vec2f,
   glyph: u32,
   color: u32,
}

struct AtlasGlyph {
   @align(16)
   rect: vec4u,
}

@group(0) @binding(0) var<uniform> glyph_data: array<Glyph, max_glyph_count>;
@group(1) @binding(0) var atlas_texture: texture_2d<f32>;
@group(1) @binding(1) var<uniform> atlas_data: array<AtlasGlyph, max_atlas_glyph_count>;
@group(2) @binding(0) var<uniform> model_transform: mat3x3f;
@group(3) @binding(0) var<uniform> scene_transform: mat3x3f;
@group(3) @binding(1) var image_sampler: sampler;

@vertex
fn main_vs(
   @builtin(instance_index) glyph_index: u32,
   @location(0) position: vec2f,
) -> Vertex {
   let data = glyph_data[glyph_index];
   let atlas_rect = atlas_data[data.glyph].rect;
   let local_position = floor(data.position + position * vec2f(atlas_rect.zw));
   let scene_position = scene_transform * model_transform * vec3f(local_position, 1.0);

   let atlas_size = vec2f(textureDimensions(atlas_texture));
   let normalized_rect = vec4f(atlas_rect) / vec4f(atlas_size, atlas_size);
   let uv = normalized_rect.xy + position * vec2f(normalized_rect.zw);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, 0.0, 1.0);
   vertex.glyph_index = glyph_index;
   vertex.uv = uv;
   return vertex;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   var color = unpack4x8unorm(glyph_data[vertex.glyph_index].color);
   let alpha = textureSample(atlas_texture, image_sampler, vertex.uv).r;
   color.a *= alpha;
   color *= vec4f(vec3f(color.a), 1.0);
   return color;
}

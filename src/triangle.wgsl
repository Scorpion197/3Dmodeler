struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    normal: mat4x4<f32>,
    color: vec4<f32>,
    options: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> object: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
) -> VertexOutput {
    var output: VertexOutput;

    output.position = object.mvp * vec4<f32>(position, 1.0);
    output.normal = (object.normal * vec4<f32>(normal, 0.0)).xyz;
    output.world = (object.model * vec4<f32>(position, 1.0)).xyz;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.4, 0.8, 0.6));

    let shade =
        0.3 + 0.7 * max(dot(normalize(input.normal), light), 0.0);

    var color = object.color.rgb;

    // Selected objects receive a gold tint.
    if object.options.x > 0.5 {
        color = mix(color, vec3<f32>(1.0, 0.65, 0.08), 0.7);
    }

    // Draw a checker pattern on the floor.
    if object.options.y > 0.5 {
        let cell = floor(input.world.x) + floor(input.world.z);
        let alternating = cell - 2.0 * floor(cell / 2.0);

        color *= 0.85 + 0.15 * alternating;
    }

    return vec4<f32>(color * shade, 1.0);
}

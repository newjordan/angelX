// Exact box integration of the FP16 accumulation target; no sharpening, gamma
// change or tone curve. Clamp/8-bit quantization happens only after composition.
@group(0) @binding(0) var accumulated: texture_2d<f32>;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vertex(@builtin(vertex_index) id:u32) -> Vertex {
    let corners = array<vec2f,3>(vec2f(0.,0.),vec2f(2.,0.),vec2f(0.,2.));
    let uv = corners[id];
    return Vertex(vec4f(uv.x*2.-1.,1.-uv.y*2.,0.,1.),uv);
}
@fragment fn fragment(in:Vertex) -> @location(0) vec4f {
    let dimensions = vec2f(textureDimensions(accumulated));
    let grid = clamp(u32(round(abs(dpdx(in.uv.x))*dimensions.x)),1u,2u);
    let origin = vec2i(in.position.xy)*i32(grid);
    var color=vec4f(0.);
    for(var y=0u;y<grid;y++){for(var x=0u;x<grid;x++){
        color += textureLoad(accumulated,origin+vec2i(i32(x),i32(y)),0);
    }}
    return color/f32(grid*grid);
}

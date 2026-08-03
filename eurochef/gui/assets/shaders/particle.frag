precision mediump float;

in vec2 f_uv;
in vec4 f_colour;

uniform sampler2D u_texture;
uniform int u_hasTexture;

out vec4 o_color;
void main() {
    vec4 sampleColour;
    if (u_hasTexture != 0) {
        sampleColour = texture(u_texture, f_uv);
    } else {
        vec2 centered = f_uv * 2.0 - 1.0;
        float radiusSquared = dot(centered, centered);
        if (radiusSquared > 1.0) discard;
        float radial = smoothstep(1.0, 0.0, radiusSquared);
        sampleColour = vec4(vec3(radial), radial);
    }
    o_color = sampleColour * f_colour;
    if (o_color.a <= 0.001) discard;
}

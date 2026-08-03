#extension GL_ARB_explicit_attrib_location : enable
precision mediump float;

layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec2 a_uv;
layout (location = 2) in vec3 a_instancePosition;
layout (location = 3) in vec3 a_instanceScale;
layout (location = 4) in vec3 a_instanceRotation;
layout (location = 5) in vec4 a_instanceColour;

out vec2 f_uv;
out vec4 f_colour;

uniform mat4 u_view;
uniform mat4 u_emitterModel;
uniform mat4 u_billboardRotation;

void main() {
    float c = cos(a_instanceRotation.z);
    float s = sin(a_instanceRotation.z);
    vec2 local = a_pos.xy * a_instanceScale.xy;
    local = mat2(c, -s, s, c) * local;

    vec3 worldCenter = (u_emitterModel * vec4(a_instancePosition, 1.0)).xyz;
    vec3 billboardOffset = (u_billboardRotation * vec4(local, 0.0, 0.0)).xyz;

    f_uv = a_uv;
    f_colour = a_instanceColour;
    gl_Position = u_view * vec4(worldCenter + billboardOffset, 1.0);
}

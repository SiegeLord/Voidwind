#version 330 core
uniform float time;
in vec3 varying_pos;

layout(location = 0) out vec3 position_buffer;
layout(location = 1) out vec3 normal_buffer;
layout(location = 2) out vec4 albedo_buffer;

uniform sampler2D al_tex;

void main()
{
    position_buffer = varying_pos;
    normal_buffer = vec3(0, 1, 0);
    float v = dot(normalize(vec2(1., 1.)), vec2(varying_pos.x + cos(time) * sin(varying_pos.z), varying_pos.z));
	albedo_buffer = vec4(0., 0., sin(v) * 0.5 + 0.5, 1.0);
}

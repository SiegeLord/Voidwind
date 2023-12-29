#version 330 core
uniform float time;
in vec3 varying_pos;

layout(location = 0) out vec3 position_buffer;
layout(location = 1) out vec4 normal_buffer;
layout(location = 2) out vec4 albedo_buffer;

uniform sampler2D al_tex;

#define PI 3.14159265359

void main()
{
    position_buffer = varying_pos;
    //float v = dot(normalize(vec2(1., 1.)), vec2(varying_pos.x + cos(time) * sin(varying_pos.z), varying_pos.z));
    
    float u = 2 * varying_pos.z + varying_pos.x;
    float v = varying_pos.z + 2 * varying_pos.x;

    float var2 = sin((u / 8 * 2. * PI) / 2. + time / 4.);
    float var = sin((v / 8 * 2. * PI) + var2 + time / 2.);
    vec3 normal = normalize(vec3(0.5 * sign(var) * pow(abs(var), 0.5), 1., var2));
    //vec3 normal = normalize(vec3(0., 1., 0.));
    normal_buffer = vec4(normal, 1.);
    albedo_buffer = vec4(0.1, 0.1, 0.8, 1.);
}

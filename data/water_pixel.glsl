#version 330 core
uniform float time;
in vec3 varying_pos;

layout(location = 0) out vec3 position_buffer;
layout(location = 1) out vec4 normal_buffer;
layout(location = 2) out vec4 albedo_buffer;

uniform sampler2D al_tex;

void main()
{
    position_buffer = varying_pos;
    //float v = dot(normalize(vec2(1., 1.)), vec2(varying_pos.x + cos(time) * sin(varying_pos.z), varying_pos.z));
    float var2 = sin(varying_pos.z / 3. + time / 4.);
    float var = sin(varying_pos.x + var2 + time / 3.);
    vec3 normal = normalize(vec3(0.5 * sign(var) * pow(abs(var), 0.5), 1., var2));
    //vec3 normal = normalize(vec3(0., 1., 0.));
    normal_buffer = vec4(normal, 1.);
    albedo_buffer = vec4(0., 0., 0.7, 1.);
}

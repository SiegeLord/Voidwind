#version 330 core
in vec2 varying_texcoord;
layout(location = 0) out vec4 out_color;

//uniform vec3 camera_pos;

uniform sampler2D al_tex; // Light buffer.
uniform sampler2D position_buffer;
uniform sampler2D normal_buffer;
uniform sampler2D albedo_buffer;

void main()
{
	vec3 pos = texture(position_buffer, varying_texcoord).xyz;
	vec3 normal = texture(normal_buffer, varying_texcoord).xyz;
	vec4 color = vec4(texture(albedo_buffer, varying_texcoord).rgb, 1);
	vec4 light_color = vec4(texture(al_tex, varying_texcoord).rgb, 1);
    out_color = light_color * color;
    //out_color = vec4(mod(pos.xyz, 1), 1);
    //out_color = vec4(normal, 1);
}

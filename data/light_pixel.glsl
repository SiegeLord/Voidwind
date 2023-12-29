#version 330 core
out vec4 color;

uniform vec4 light_color;
uniform vec3 light_pos;
uniform vec2 buffer_size;

uniform sampler2D position_buffer;
uniform sampler2D normal_buffer;

void main()
{
    vec2 texcoord = gl_FragCoord.xy / buffer_size;
	vec3 pos = texture(position_buffer, texcoord).xyz;
    vec3 normal = texture(normal_buffer, texcoord).xyz;

    vec3 ray_dir = pos - light_pos;
    //if (dot(ray_dir, normal) > 0.)
    //    discard;

    vec3 diff = (light_pos - pos) / 10.;
    float dist_sq = dot(diff, diff);
	color = light_color / (1 + dist_sq * dist_sq);
}

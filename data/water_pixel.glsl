#version 330 core
in vec4 varying_pos;
uniform float time;
out vec4 color;

void main()
{
    float v = dot(normalize(vec2(1., 1.)), vec2(varying_pos.x + cos(time) * sin(varying_pos.z), varying_pos.z));
	color = vec4(0., 0., sin(v) * 0.5 + 0.5, 1.0);
}

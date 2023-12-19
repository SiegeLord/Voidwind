#version 330 core
layout(location = 0) in vec4 al_pos;
layout(location = 1) in vec4 al_color;
layout(location = 2) in vec2 al_texcoord;
layout(location = 3) in vec3 al_user_attr_0;  // normal
varying vec4 varying_color;
varying vec2 varying_texcoord;
uniform mat4 al_projview_matrix;

void main()
{
   vec3 normal = al_user_attr_0;
   float f = dot(normal, normalize(vec3(1.0, 1.0, 1.0)));
   varying_color = al_color * vec4(f, f, f, 1.0);
   varying_texcoord = al_texcoord;
   gl_Position = al_projview_matrix * al_pos;
}

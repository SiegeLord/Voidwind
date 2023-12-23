#version 330 core
layout(location = 0) in vec4 al_pos;
varying vec4 varying_pos;
uniform mat4 al_projview_matrix;

void main()
{
   varying_pos = al_pos;
   gl_Position = al_projview_matrix * al_pos;
}

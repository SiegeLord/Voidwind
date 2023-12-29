attribute vec4 al_pos;

uniform mat4 al_projview_matrix;

varying vec3 varying_pos;

void main()
{
   varying_pos = al_pos.xyz;
   gl_Position = al_projview_matrix * al_pos;
}

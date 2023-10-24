#version 430 core

in vec2 TexCoords;

out vec4 FragColor;

uniform sampler2D tex;

void main() { 
    vec4 texCol = texture(tex, TexCoords).rgba;
    FragColor = vec4(texCol);
}
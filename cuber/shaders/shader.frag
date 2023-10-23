#version 430 core

in vec2 TexCoords;

out vec4 FragColor;

uniform sampler2D tex;

void main() { 
    vec3 texCol = texture(tex, TexCoords).rgb;
    FragColor = vec4(texCol, 1.0);
    // FragColor = vec4(1.0, 1.0, 0.0, 1.0); 
}
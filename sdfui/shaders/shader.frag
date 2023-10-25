#version 430 core

in vec2 TexCoords;

out vec4 FragColor;

uniform sampler2D sdf;
uniform sampler2D present;
uniform sampler2D weights;

void main() { 
    float d = texture(sdf, TexCoords).r;
    vec4 texColor = texture(present, TexCoords);
    vec4 emptyColor = vec4(0.0, 0.0, 0.0, 0.0);

    vec4 finalColor = mix(
        texColor,
        emptyColor,
        smoothstep(-1.0, 1.0, d / fwidth(d))
    );

    // FragColor = finalColor;
    FragColor = finalColor;
}
#version 460

struct Text {
  vec2 pos;
  vec2 dim;
  uint offset;
  int padding[3];
  vec4 color;
};

struct Shape4 {
  vec4 data;
  vec4 color;
};

struct Command {
  int kind;
  int idx;
  int fun;
  float extra;
  vec4 data;
};

uniform uint commands_length;
uniform uint shape_count;

layout(std430, binding = 0) buffer Commands {
  Command commands[];
};


layout(std430, binding = 1) buffer Texts {
  Text texts[];
}; 

layout(std430, binding = 2) buffer TextData {
  uint textdata[];
};


layout(std430, binding = 3) buffer Shapes4 {
  Shape4 shapes4[];
};

// unions
float smoothMax(float a, float b, float k) {
  return log(exp(k * a) + exp(k * b)) / k;
}

float smoothMin(float a, float b, float k) {
  return -smoothMax(-a, -b, k);
}

// primitives
float sdCircle(vec2 c, float r) {
  return length(gl_FragCoord.xy - c) - r;
}


float sdBox(vec2 center, vec2 size) {
  vec2 d = abs(gl_FragCoord.xy - center) - size;
  return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdText(vec2 pos, vec2 dim, uint offset) {
  uint x_index = uint(gl_FragCoord.x - pos.x);
  uint y_index = uint(dim.y - (gl_FragCoord.y - pos.y) - 1);
  uint index = offset + x_index + y_index * uint(dim.x);
  
  uint value_packed = textdata[index / 4];
  uint value_int = ( value_packed >> (8 * (index % 4))) & 0xFF;
  float value = (-float(value_int)) / 255.0;
  if (value == 0) { return 20.0; } else { return value; }
}


out vec4 FragColor;

void main() {
  vec4 bgColor = vec4(0.0, 0.0, 0.0, 0.0);
  float d = 1e10;

  vec4 finalColor = vec4(0.0, 0.0, 0.0, 1.0);
  float totalWeight = 0.0;
  

  for (int i = 0; i < commands_length; i++) {
    Command com = commands[i];
    float dt = 0.0;

    switch (com.kind) {
      case 1:
        Shape4 sc = shapes4[com.idx];
        dt = sdCircle(sc.data.xy, sc.data.z);
        break;
      case 2:
        Shape4 sb = shapes4[com.idx];
        dt = sdBox(sb.data.xy, sb.data.zw);
        break;
      case 4:
        Text t = texts[com.idx];
        vec2 text_top_right = t.pos + t.dim.xy;
        if (gl_FragCoord.x >= t.pos.x && 
          gl_FragCoord.x <= text_top_right.x && 
          gl_FragCoord.y >= t.pos.y && 
          gl_FragCoord.y <= text_top_right.y) 
        {
          dt = sdText(t.pos, t.dim, t.offset);
          vec4 color = texts[com.idx].color;
          float w = 1.0 / (1.0 + exp(10.0 * dt));
          totalWeight += w;
          finalColor += w * color;
        }
        break;
      case 69:
        finalColor = mix(com.data, finalColor, smoothstep(
          0.0, com.extra, abs(d)));
        break; 
      default:
        FragColor = vec4(1.0, 0.0, 1.0, 1.0);
        return;
    }

    if (com.kind > 0 && com.kind < 3) {
      vec4 color = shapes4[com.idx].color; 
      float w = 1.0 / (1.0 + exp(10.0 * dt));
      totalWeight += w;
      finalColor += w * color;
    }

   
    switch (com.fun) {
      case 1:
        d = min(d, dt); break;
      case 2:
        d = max(d, dt); break;
      case 3:
        d = smoothMin(d, dt, com.extra); break;
      case 4:
        d = smoothMax(d, dt, com.extra); break;
      default:
        break;
    }
  }

  if ( totalWeight > 0.0 ) {
    finalColor /= totalWeight;
  }
  
  finalColor = mix(
    finalColor,
    bgColor,
    smoothstep(-1.0, 1.0, d / fwidth(d))
  );
  
  FragColor = finalColor;
}
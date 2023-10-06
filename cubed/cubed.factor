USING: accessors alien arrays classes.struct colors combinators 
kernel math multiline opengl opengl.gl opengl.shaders sequences
specialized-arrays
specialized-arrays.instances.alien.c-types.float specialized-vectors ;
QUALIFIED-WITH: alien.c-types c
IN: cubed

! TODO: Add them to Factor
CONSTANT: GL_SHADER_STORAGE_BUFFER 0x90d2

STRING: cubed-vertex-shader
#version 460

void main() {
  float x = -1.0 + float((gl_VertexID & 1) << 2);
  float y = -1.0 + float((gl_VertexID & 2) << 1);
  gl_Position = vec4(x, y, 0.0, 1.0);
}
;

STRING: cubed-fragment-shader
#version 460

struct Shape4 {
  vec4 data;
  vec4 color;
};

struct Command {
  int kind;
  int idx;
  int fun;
  float extra;
};

uniform uint commands_length;
uniform uint shape_count;

layout(std430, binding = 0) buffer Commands {
  Command commands[];
};

layout(std430, binding = 1) buffer Shapes4 {
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
        FragColor = vec4(1.0, 0.0, 1.0, 1.0);
        return;
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

//  if (d < 0.0) {
//    FragColor = vec4(1.0, 0.0, 0.0, 1.0);
//  }
}
;

: color>float-array ( color -- float-array )
  >rgba-components 4array [ ] float-array{ } map-as ;


PACKED-STRUCT: Shape4
  { data c:float[4] }
  { color c:float[4] } ;

PACKED-STRUCT: Command
  { kind c:int }
  { idx c:int }
  { fun c:int }
  { extra c:float } ;

SPECIALIZED-VECTORS: Command Shape4 ;
SPECIALIZED-ARRAYS:  Command Shape4 ;

: <c:circle> ( x y r color -- circle ) 
  [ 0 4array [ ] float-array{ } map-as ] dip 
  color>float-array Shape4 boa ;

: <c:box> ( x y w h color -- box )
  [ 4array [ ] float-array{ } map-as ] dip
  color>float-array Shape4 boa ;

: <command> ( kind idx -- command ) 1 0 Command boa ;

SYMBOLS: umin umax usmin usmax ;

: =union ( command sym -- command ) {
  { umin [ 1 ] }
  { umax [ 2 ] }
  { usmin [ 3 ] }
  { usmax [ 4 ] }
} case >>fun ;

TUPLE: merge fun value ;

: <merge> ( fun value -- merge ) merge boa ;

: command-add-merge ( command merge -- command ) 
  [ fun>> ] [ value>> ] bi
  [ =union ] dip >>extra ;


TUPLE: buffers
  command-ssbo
  shapes4-ssbo ;

: <buffers> ( -- buffers ) 
  create-gl-buffer
  create-gl-buffer
  buffers boa ;

: cache=>buffer ( cache buffer type -- ) 
  rot [ length swap c:heap-size * ] keep GL_DYNAMIC_COPY glNamedBufferData ;

: bind-buffers ( buffers -- ) 
  [ [ GL_SHADER_STORAGE_BUFFER 0 ] dip command-ssbo>> glBindBufferBase ] 
  [ [ GL_SHADER_STORAGE_BUFFER 1 ] dip shapes4-ssbo>> glBindBufferBase ] bi ;

TUPLE: cubed-cache 
  commands 
  shapes4 ;

: <cubed-cache> ( -- cache ) 
  Command-vector{ } clone
  Shape4-vector{ } clone 
  cubed-cache boa ;

TUPLE: cubed-ctx
  program cache buffers ;

: <cubed-ctx> ( -- ctx ) 
  cubed-vertex-shader cubed-fragment-shader <simple-gl-program>
  <cubed-cache> 
  <buffers> 
  cubed-ctx boa ;

: cubed-ctx-record ( ctx -- )
  [ drop <cubed-cache> ] change-cache drop ;

: cubed-ctx-submit-commands ( ctx -- )
  [ cache>> ] [ buffers>> ] bi
  [ [ commands>> ] dip command-ssbo>> Command cache=>buffer ] 
  [ [ shapes4>>  ] dip shapes4-ssbo>> Shape4  cache=>buffer ] 2bi ;

: cubed-ctx-bind-buffers ( ctx -- )
  buffers>> bind-buffers ;

: cubed-ctx-program ( ctx -- ) 
  dup program>> [ over cache>> commands>> length 
    [ dup "commands_length" glGetUniformLocation ] dip glProgramUniform1ui
    GL_TRIANGLES 0 4 glDrawArrays 
  ] with-gl-program drop ;

: cubed-ctx-render ( ctx -- ) { 
    [ cubed-ctx-submit-commands ]
    [ cubed-ctx-bind-buffers ]
    [ cubed-ctx-program ]
  } cleave ;

! : cubed-ctx-add-shape4 ( shape kind ctx -- command ) 
!   cache>> [ shapes4>> ] [ commands>> ] bi ! s i ss cs
!   [ dup length>> swapd <command> ] dip push push ; 

: cubed-ctx-add-shape4 ( ctx shape kind merge/f -- )
  [ rot cache>> [ shapes4>> ] [ commands>> ] bi ! s k ss cs 
    [ dup length>> swapd <command> ] dip ! s ss c cs
  ] dip ! s ss c cs m -> s ss cs c m -> s ss cs c
  swapd dup [ command-add-merge ] [ drop ] if 
  swap push push ; ! s ss cs c


! : cubed-ctx-add-circle ( circle ctx -- command )
!   [ 1 ] dip cubed-ctx-add-shape4 ;

! : cubed-ctx-add-box ( box ctx -- command )
!  [ 2 ] dip cubed-ctx-add-shape4 ;

: cubed-ctx>circle ( ctx x y r c m/f -- ) ! ctx x y r c m ->
  [ <c:circle> 1 ] dip cubed-ctx-add-shape4 ;

: cubed-ctx>box ( ctx x y w h c m/f -- )
  [ <c:box> 2 ] dip cubed-ctx-add-shape4 ;

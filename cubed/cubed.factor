USING: accessors alien arrays classes.struct combinators kernel
math multiline opengl opengl.gl opengl.shaders sequences
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

struct Circle {
  vec2 pos;
  float r;
  float padding;
  vec4 color;
};

struct Command {
  int kind;
  int idx;
  int fun;
  int padding;
};

uniform uint commands_length;

layout(std430, binding = 0) buffer Commands {
  Command commands[];
};

layout(std430, binding = 1) buffer Circles {
  Circle circles[];
};

float sdCircle(vec2 p, float r) {
    return length(gl_FragCoord.xy - p) - r;
}


float sdBox(vec2 center, vec2 size) {
    vec2 d = abs(gl_FragCoord.xy - center) - size;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}


out vec4 FragColor;

void main() {
  FragColor = vec4(0.0, 0.0, 1.0, 1.0);
  float d = 0;
  
  for (int i = 0; i <= commands_length; i++) {
    Command com = commands[i];
    float dt = 0.0;
    if (com.kind == 0) {
      Circle c = circles[com.idx];
      dt = sdCircle(c.pos, c.r);
      d = min(d, dt);
    }
  }

  if (d < 0.0) {
    FragColor = vec4(1.0, 0.0, 0.0, 1.0);
  }
}
;


PACKED-STRUCT: Circle 
  { position c:float[2] }
  { radius c:float }
  { padding c:float }
  { color c:float[4] } ;


PACKED-STRUCT: Command
  { kind c:int }
  { idx c:int }
  { fun c:int }
  { padding c:int } ;

SPECIALIZED-VECTORS: Circle Command ;
SPECIALIZED-ARRAYS:  Circle Command ;

: <c:circle> ( pos r color -- circle ) [ 0 ] dip Circle boa ;
: <circle-command> ( idx -- command ) [ 0 ] dip 0 0 Command boa ;


TUPLE: buffers
  command-ssbo
  circle-ssbo ;

: <buffers> ( -- buffers ) 
  create-gl-buffer
  create-gl-buffer
  buffers boa ;

: cache=>buffer ( cache buffer type -- ) 
  rot [ length swap c:heap-size * ] keep GL_DYNAMIC_COPY glNamedBufferData ;

: bind-buffers ( buffers -- ) 
  [ [ GL_SHADER_STORAGE_BUFFER 0 ] dip command-ssbo>> glBindBufferBase ] 
  [ [ GL_SHADER_STORAGE_BUFFER 1 ] dip circle-ssbo>>  glBindBufferBase ] bi ;

TUPLE: cubed-cache 
  commands 
  circles ;

: <cubed-cache> ( -- cache ) 
  Command-vector{ } clone
  Circle-vector{ } clone 
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
  [ [ circles>>  ] dip circle-ssbo>>  Circle  cache=>buffer ] 2bi ;

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

: cubed-ctx-add-circle ( circle ctx -- ) 
  cache>> [ circles>> ] [ commands>> ] bi
  [ dup length>> <circle-command> ] dip push push ;

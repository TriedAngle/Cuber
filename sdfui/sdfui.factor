USING: accessors alien arrays classes.struct colors combinators
generalizations kernel math multiline opengl opengl.gl
opengl.shaders sdfui.fonts sdfui.utils sequences specialized-arrays
specialized-arrays.instances.alien.c-types.float 
specialized-arrays.instances.alien.c-types.int specialized-vectors ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui

! TODO: Add them to Factor
CONSTANT: GL_SHADER_STORAGE_BUFFER 0x90d2

STRING: sdfui-vertex-shader
#version 460

void main() {
  float x = -1.0 + float((gl_VertexID & 1) << 2);
  float y = -1.0 + float((gl_VertexID & 2) << 1);
  gl_Position = vec4(x, y, 0.0, 1.0);
}
;

STRING: sdfui-fragment-shader
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

// index like this
// val = text[index / 4]
// val = val >> (value >> (8 * (index % 4))) & 0xFF;
// val = float(intValue) / 255.0;
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
          // FragColor = vec4(1.0, 1.0, 0.0, 1.0);
          // return;
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
;

: color>float-array ( color -- float-array )
  >rgba-components 4array [ ] float-array{ } map-as ;


PACKED-STRUCT: Shape4
  { data c:float[4] }
  { color c:float[4] } ;

PACKED-STRUCT: Text
  { pos c:float[2] }
  { dim c:float[2] }
  { offset c:uint }
  { padding c:int[3] }
  { color c:float[4] } ;

PACKED-STRUCT: Command
  { kind c:int }
  { idx c:int }
  { fun c:int }
  { extra c:float }
  { data c:float[4] } ;

SPECIALIZED-VECTORS: Command Shape4 Text ;


: <c:text> ( x y w h offset color -- text ) 
  [ [ 2array [ ] float-array{ } map-as ] 2dip 
    2array [ ] float-array{ } map-as
  ] 2dip color>float-array [ int-array{ 0 0 0 } ] dip Text boa ;

: <c:circle> ( x y r color -- circle ) 
  [ 0 4array [ ] float-array{ } map-as ] dip 
  color>float-array Shape4 boa ;

: <c:box> ( x y w h color -- box )
  [ 4array [ ] float-array{ } map-as ] dip
  color>float-array Shape4 boa ;

: <command> ( k i f e d -- command ) Command boa ;
: <!data-command> ( k i f e -- command ) float-array{ 0 0 0 0 } Command boa ; 

: <s-command> ( kind idx -- command ) 1 0 <!data-command> ;

: <text-command> ( idx -- command ) [ 4 ] dip 1 0 <!data-command> ;

: <outline-command> ( thicc color -- command ) 
  [ 69 0 0 ] 2dip color>float-array <command>  ;

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
  dup [ 
    [ fun>> ] [ value>> ] bi
    [ =union ] dip >>extra
  ] [ drop ] if ;


TUPLE: cached-text 
  { x float } { y float } 
  { rasta font-string }
  color ; 

: <cached-text> ( x y rasta color -- font ) cached-text boa ;

TUPLE: buffers
  command-ssbo
  texts-ssbo
  textdata-ssbo
  shapes4-ssbo ;

: <buffers> ( -- buffers ) 
  create-gl-buffer
  create-gl-buffer
  create-gl-buffer
  create-gl-buffer
  buffers boa ;

: cache=>buffer ( cache buffer type -- ) 
  rot [ length swap c:heap-size * ] keep GL_DYNAMIC_COPY glNamedBufferData ;

: bind-buffers ( buffers -- ) {
  [ [ GL_SHADER_STORAGE_BUFFER 0 ] dip command-ssbo>>  glBindBufferBase ] 
  [ [ GL_SHADER_STORAGE_BUFFER 1 ] dip texts-ssbo>>    glBindBufferBase ]
  [ [ GL_SHADER_STORAGE_BUFFER 2 ] dip textdata-ssbo>> glBindBufferBase ]
  [ [ GL_SHADER_STORAGE_BUFFER 3 ] dip shapes4-ssbo>>  glBindBufferBase ]
} cleave ;

TUPLE: sdfui-cache 
  commands 
  shapes4 
  cached-texts ;

: <sdfui-cache> ( -- cache ) 
  Command-vector{ } clone
  Shape4-vector{ } clone 
  V{ } clone
  sdfui-cache boa ;

TUPLE: sdfui-ctx
  program
  sdfui-fonts
  cache
  buffers ;

: <sdfui-ctx> ( -- ctx ) 
  sdfui-vertex-shader sdfui-fragment-shader <simple-gl-program> 
  <sdfui-fonts>
  <sdfui-cache>
  <buffers> 
  sdfui-ctx boa ;

: sdfui-record ( ctx -- )
  [ drop <sdfui-cache> ] change-cache drop ;

: sdfui-submid-textdata ( ctx -- )
  dup cache>> cached-texts>> 0 [ rasta>> rasta>> data>> length + 4 round-to ] reduce
  over buffers>> textdata-ssbo>> swap f GL_DYNAMIC_DRAW glNamedBufferData
  [ cache>> ] [ buffers>> ] bi Text-vector{ } clone
  [| cache buffers texts | 
    0 :> offset
    cache cached-texts>> [ 
      { [ [ x>> ] [ y>> ] bi ]
        [ rasta>> rasta>> [ width>> ] [ height>> ] bi ]
        [ drop offset ]
        [ color>> ]
        [ rasta>> rasta>> data>> [ length ] keep 
          [ buffers textdata-ssbo>> offset ] 2dip
          glNamedBufferSubData ]
        [ rasta>> rasta>> data>> length offset + 4 round-to :> offset ]
      } cleave
      <c:text> texts push
    ] each
   buffers texts-ssbo>> texts [ length Text c:heap-size * ] keep 
   GL_DYNAMIC_COPY glNamedBufferData
  ] call ; 

: sdfui-submit-commands ( ctx -- )
  dup sdfui-submid-textdata
  [ cache>> ] [ buffers>> ] bi
  [ [ commands>> ] dip command-ssbo>> Command cache=>buffer ] 
  [ [ shapes4>>  ] dip shapes4-ssbo>> Shape4  cache=>buffer ] 2bi ;

: sdfui-bind-buffers ( ctx -- )
  buffers>> bind-buffers ;

: sdfui-run-program ( ctx -- ) 
  dup program>> [ over cache>> commands>> length 
    [ dup "commands_length" glGetUniformLocation ] dip glProgramUniform1ui
    GL_TRIANGLES 0 4 glDrawArrays 
  ] with-gl-program drop ;

: sdfui-render ( ctx -- ) { 
    [ sdfui-submit-commands ]
    [ sdfui-bind-buffers ]
    [ sdfui-run-program ]
  } cleave ;

: sdfui>shape4 ( ctx shape kind merge/f -- )
  [ rot cache>> [ shapes4>> ] [ commands>> ] bi ! s k ss cs 
    [ dup length>> swapd <s-command> ] dip ! s ss c cs
  ] dip ! s ss c cs m -> s ss cs c m -> s ss cs c
  swapd command-add-merge 
  swap push push ; ! s ss cs c

: sdfui>circle ( ctx x y r c m/f -- )
  [ <c:circle> 1 ] dip sdfui>shape4 ;

: sdfui>box ( ctx x y w h c m/f -- )
  [ <c:box> 2 ] dip sdfui>shape4 ;

: sdfui>text ( ctx x y text size fonts c m/f -- ) 
  [ rot dup sdfui-fonts>> ] 5 ndip ! x y ctx sf txt s fs c m
  [ roll add-string ] 2dip ! x y ctx font-string c m
  [ [ -rotd ] dip <cached-text> ] dip -rot ! m ctx cached-text
  swap cache>> [ commands>> ] [ cached-texts>> ] bi ! m ct coms cts
  dup length [ swapd push ] dip
  <text-command> rot command-add-merge swap push ;

: sdfui>outline ( ctx thicc color -- )
  <outline-command> swap cache>> commands>> push ;

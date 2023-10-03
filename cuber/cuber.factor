! Copyright (C) 2023 Your name.
! See https://factorcode.org/license.txt for BSD license.
USING: accessors colors combinators game.input
game.input.scancodes game.loop game.worlds io kernel literals
math multiline namespaces opengl opengl.gl opengl.shaders
opengl.textures prettyprint sequences
specialized-arrays.instances.alien.c-types.float ui
ui.gadgets.worlds ui.pixel-formats ;
IN: cuber

STRING: compute-shader
#version 450

layout ( local_size_x = 8, local_size_y = 8, local_size_z = 1 ) in ;

layout ( rgba32f, binding = 0 ) uniform image2D imgOutput ;

void main ( ) { 
  ivec2 texelCoord = ivec2( gl_GlobalInvocationID.xy ) ;

  vec4 value = vec4( 0.5, 0.0, 0.25, 1.0 ) ;
  
  imageStore( imgOutput, texelCoord, value ) ;
} 
;

STRING: vertex-shader
#version 450

layout ( location = 0 ) in vec3 aPos ;
layout ( location = 1 ) in vec2 aTexCoords ;

out vec2 TexCoords ;

void main ( ) { 
  TexCoords = aTexCoords ;
  gl_Position = vec4( aPos, 1.0 ) ; 
}
;

STRING: fragment-shader
#version 450

in vec2 TexCoords ;

out vec4 FragColor ;

uniform sampler2D tex ;

void main ( ) { 
  vec3 texCol = texture( tex, TexCoords ).rgb ;
  FragColor = vec4( texCol, 1.0 ) ; 
}
;

CONSTANT: sizeof<float> 4 

: dim|| ( dim -- x y ) 
  [ first ] [ second ] bi ;


: <empty-image> ( -- image ) GL_TEXTURE_2D create-texture ;

: <present-image> ( dim  -- image )
  <empty-image> dup -rot { 
    [ swap [ 1 GL_RGBA32F ] dip dim|| glTextureStorage2D ]
    [ GL_TEXTURE_WRAP_S GL_CLAMP_TO_EDGE glTextureParameteri ]
    [ GL_TEXTURE_WRAP_T GL_CLAMP_TO_EDGE glTextureParameteri ]
    [ GL_TEXTURE_MAG_FILTER GL_LINEAR glTextureParameteri ]
    [ GL_TEXTURE_MIN_FILTER GL_LINEAR glTextureParameteri ]
  } cleave ;

: bind-image ( image -- ) 
  0 swap 0 GL_FALSE 0 GL_READ_WRITE GL_RGBA32F glBindImageTexture ;


INITIALIZED-SYMBOL: present-verts [ float-array{ 
  -1.0  1.0  0.0  0.0  1.0
  -1.0 -1.0  0.0  0.0  0.0
   1.0  1.0  0.0  1.0  1.0
   1.0 -1.0  0.0  1.0  0.0 
 } ]


: present-verts* ( -- verts ) present-verts get ;

TUPLE: cuber < game-world
  program 
  present-program { present-image fixnum initial: -1 } vao vbo ;

: resize-present ( world -- )
  [ present-image>> delete-texture ]
  [ [ dim>> <present-image> dup bind-image ] keep present-image<< ] bi ;

M: cuber begin-game-world
  compute-shader <compute-program> >>program
  vertex-shader fragment-shader <simple-gl-program> >>present-program

  create-vertex-array >>vao
  create-gl-buffer >>vbo

  dup vbo>> present-verts* length sizeof<float> * present-verts* GL_STATIC_DRAW glNamedBufferData
  
  dup [ vao>> 0 ] [ vbo>> ] bi 0 5 sizeof<float> * glVertexArrayVertexBuffer
 
  dup vao>> {
    [ 0 glEnableVertexArrayAttrib ]
    [ 0 3 GL_FLOAT GL_FALSE 0 glVertexArrayAttribFormat ]
    [ 0 0 glVertexArrayAttribBinding ]

    [ 1 glEnableVertexArrayAttrib ]
    [ 1 2 GL_FLOAT GL_FALSE 3 4 * glVertexArrayAttribFormat ]
    [ 1 0 glVertexArrayAttribBinding ]
  } cleave

  dup resize-present
  drop ;

M: cuber end-game-world { 
  [ program>> delete-gl-program ]
  [ present-program>> delete-gl-program ]
  [ present-image>> delete-texture ]
  [ vbo>> delete-gl-buffer ]
  [ vao>> delete-vertex-array ]
} cleave ;

:: handle-tick-input ( world -- )
  read-keyboard keys>> :> keys
  key-escape keys nth [ world close-window ] when ;

M: cuber tick-game-world {
  [ handle-tick-input ]
} cleave ;

: gl-clear-depth ( -- ) GL_COLOR_BUFFER_BIT GL_DEPTH_BUFFER_BIT bitor glClear ;

M: cuber draw-world*
  dup program>> [
      over dim>> dim|| [ ] bi@ 1 glDispatchCompute
      GL_SHADER_IMAGE_ACCESS_BARRIER_BIT glMemoryBarrier 
      drop 
  ] with-gl-program

  gl-clear-depth
  
  dup present-program>> [
    dup "tex" glGetUniformLocation 0 glProgramUniform1i
    dup 
      [ present-image>> 0 swap glBindTextureUnit ]
      [ vao>> glBindVertexArray ] bi
    GL_TRIANGLE_STRIP 0 4 glDrawArrays
  ] with-gl-program
  drop ;

M: cuber resize-world
  dup resize-present
  [ 0 0 ] dip dim>> dim|| glViewport ;

GAME: cuber-game {
  { world-class cuber }
  { title "Cuber" }
  { pixel-format-attributes {
    windowed double-buffered
    T{ depth-bits { value 24 } }
  } }
  { use-game-input? t }
  { grab-input? t }
  { pref-dim { 1000 1000 } }
  { tick-interval-nanos $[ 60 fps ] }
} ;

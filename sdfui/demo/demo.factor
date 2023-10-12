USING: accessors alien alien.c-types alien.strings alien.syntax
colors combinators game.input game.input.scancodes game.loop
game.worlds io.encodings.utf8 kernel literals namespaces
opengl.gl opengl.gl.windows prettyprint sdfui sequences
specialized-arrays.instances.alien.c-types.float
specialized-arrays.instances.alien.c-types.int ui
ui.gadgets.worlds ui.pixel-formats windows.opengl32
windows.types ;
IN: sdfui.demo

TUPLE: demo < game-world 
  ui-ctx ;

DEFER: reset-gl-context

M: demo begin-game-world 
  <sdfui-ctx> >>ui-ctx 
  dup handle>> hDC>> reset-gl-context
  drop ;

:: handle-tick-input ( world -- )
  read-keyboard keys>> :> keys
  key-escape keys nth [ world close-window ] when 
  key-space keys nth [ world ui-ctx>> [ . ] with-global ] when ;

M: demo tick-game-world {
  [ handle-tick-input ]
} cleave ;

M: demo draw-world*
  dup ui-ctx>> {
    [ sdfui-record ]
    [ 300 500 100 COLOR: red f sdfui>circle ]
    [ 900 300 60 COLOR: blue f sdfui>circle ]
    [ 300 350 100 50 COLOR: red usmin 0.2 <merge> sdfui>box ]
    [ 1.5 COLOR: white sdfui>outline ]
    [ 100 600 "hello 猫🐱 044 XD" 42 
      { "Arial.ttf" "msyh.ttc" "seguiemj.ttf" } 
      COLOR: white f sdfui>text ]
    [ 200 300 "i am in pain owo" 42 
     { "comici.ttf" } 
     COLOR: white f sdfui>text ]
    [ sdfui-render ]
  } cleave
  drop ;

GAME: demo-ui-game {
  { world-class demo }
  { title "SDF UI Library" }
  { pixel-format-attributes {
    windowed double-buffered
    T{ depth-bits { value 24 } }
  } }
  { use-game-input? t }
  { grab-input? f }
  { pref-dim { 1280 720 } }
  { tick-interval-nanos $[ 60 fps ] }
} ;

MAIN: demo-ui-game

<<
CONSTANT: WGL_CONTEXT_MAJOR_VERSION_ARB           0x2091
CONSTANT: WGL_CONTEXT_MINOR_VERSION_ARB           0x2092
CONSTANT: WGL_CONTEXT_LAYER_PLANE_ARB             0x2093
CONSTANT: WGL_CONTEXT_FLAGS_ARB                   0x2094
CONSTANT: WGL_CONTEXT_PROFILE_MASK_ARB            0x9126

CONSTANT: WGL_CONTEXT_DEBUG_BIT_ARB               0x0001
CONSTANT: WGL_CONTEXT_FORWARD_COMPATIBLE_BIT_ARB  0x0002
CONSTANT: WGL_CONTEXT_CORE_PROFILE_BIT_ARB        0x00000001
CONSTANT: ERROR_INVALID_VERSION_ARB               0x2095
CONSTANT: ERROR_INVALID_PROFILE_ARB               0x2096
>>
: wgl-context-attribs-4-6-basic ( -- int-array ) int-array{ 
  $[ WGL_CONTEXT_MAJOR_VERSION_ARB 4
  WGL_CONTEXT_MINOR_VERSION_ARB 6
  WGL_CONTEXT_PROFILE_MASK_ARB WGL_CONTEXT_CORE_PROFILE_BIT_ARB
0 ] } ;

: reset-gl-context ( hDC -- )
  dup 0 wgl-context-attribs-4-6-basic
  "wglCreateContextAttribsARB" utf8 string>alien wglGetProcAddress
  HGLRC { HDC int pointer: int } cdecl alien-indirect
  wglMakeCurrent drop ;

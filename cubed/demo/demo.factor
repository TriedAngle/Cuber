! Copyright (C) 2023 Your name.
! See https://factorcode.org/license.txt for BSD license.
USING: accessors combinators cubed game.input
game.input.scancodes game.loop game.worlds kernel literals
namespaces prettyprint sequences
specialized-arrays.instances.alien.c-types.float ui
ui.gadgets.worlds ui.pixel-formats ;
USE: opengl.gl
IN: cubed.demo

TUPLE: demo < game-world 
  ui-ctx ;

M: demo begin-game-world 
  <cubed-ctx> >>ui-ctx drop ;

! :: read-ssbo ( ctx -- ) 
!   ctx buffers>> circle-ssbo>> GL_READ_ONLY glMapNamedBuffer :> mapped
!   0 2 [a..]
! ;

:: handle-tick-input ( world -- )
  read-keyboard keys>> :> keys
  key-escape keys nth [ world close-window ] when 
  key-space keys nth [ world ui-ctx>> [ . ] with-global ] when ;
 ! key-a keys nth [ world ui-ctx>> read-ssbo ] when ;

M: demo tick-game-world {
  [ handle-tick-input ]
} cleave ;

M: demo draw-world*
  dup ui-ctx>> {
    [ cubed-ctx-record ]
    [ float-array{ 300 500 } 90.0 float-array{ 1.0 0.0 0.0 1.0 } <c:circle>
      swap cubed-ctx-add-circle ]
    [ float-array{ 900 300 } 60.0 float-array{ 1.0 0.0 0.0 1.0 } <c:circle>
      swap cubed-ctx-add-circle ]   
    [ cubed-ctx-render ]
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


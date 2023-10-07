! Copyright (C) 2023 Your name.
! See https://factorcode.org/license.txt for BSD license.
USING: accessors colors combinators sdfui game.input
game.input.scancodes game.loop game.worlds kernel literals
namespaces opengl.gl prettyprint sequences
specialized-arrays.instances.alien.c-types.float ui
ui.gadgets.worlds ui.pixel-formats ;
IN: sdfui.demo

TUPLE: demo < game-world 
  ui-ctx ;

M: demo begin-game-world 
  <sdfui-ctx> >>ui-ctx drop ;

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
    [ 300 350 100 50 COLOR: blue usmin 0.2 <merge> sdfui>box ]
    [ 1.5 COLOR: white sdfui>outline ]
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


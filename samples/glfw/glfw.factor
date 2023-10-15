USING: glfw io kernel namespaces opengl.gl pressa
pressa.constants pressa.glfw prettyprint ;
IN: samples.glfw

: main ( -- )
  T{ window-attributes
    { dim { 1280 720 } }
    { title "meow" }
    { version { 4 6 } } 
  } new-window
  
  dup set-pressa-callbacks

  [| window | 
    0.2 0.3 0.3 1.0 glClearColor
    GL_COLOR_BUFFER_BIT glClear
    ! [ pressa* . ] with-global
    keyA pressed? [ [ "A pressed" . ] with-global ] when
    keyB pressed? [ [ "B pressed" . ] with-global ] when
    keyB hold? [ [ "B hold" . ] with-global ] when
    keyB released? [ [ "B released" . ] with-global ] when
    { modShift keyX } pressed? [ [ "Shift + X pressed" . ] with-global ] when

    keyEscape released? [ window t set-should-close ] when
    pressa-flush
  ] run-window drop
;


MAIN: main

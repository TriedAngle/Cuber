USING: calendar glfw io kernel namespaces opengl.gl pressa
pressa.constants pressa.glfw prettyprint threads ;
IN: samples.glfw

! this is sadly required to make it work from terminal / renderdoc
! TODO: make a macro that runs a quot on main thread
INITIALIZED-SYMBOL: keep-alive? [ t ]
: keep-alive?* ( -- ? ) keep-alive? get-global ;

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

    keyA pressed? [ [ "A pressed" . ] with-global ] when
    keyB pressed? [ [ "B pressed" . ] with-global ] when
    keyB hold? [ [ "B hold" . ] with-global ] when
    keyB released? [ [ "B released" . ] with-global ] when
    { modShift keyX } pressed? [ [ "Shift + X pressed" . ] with-global ] when

    keyEscape released? [ window t set-should-close ] when
    pressa-flush
  ] run-window drop

  
  keep-alive?* [ [ 10 milliseconds sleep windows* ] loop ] when
;


MAIN: main

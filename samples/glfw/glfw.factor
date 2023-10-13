USING: kernel opengl.gl ;
QUALIFIED-WITH: glfw glfw
IN: samples.glfw

: main ( -- )
  T{ glfw:window-attributes
    { dim { 1280 720 } }
    { title "meow" }
    { version { 4 6 } } 
  } glfw:new-window
  [ drop 
    0.2 0.3 0.3 1.0 glClearColor
    GL_COLOR_BUFFER_BIT glClear
  ] glfw:run-window drop
;


MAIN: main

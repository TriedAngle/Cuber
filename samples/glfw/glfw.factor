USING: alien.strings glfw.ffi io.encodings.utf8 kernel opengl.gl
threads ;
IN: samples.glfw

: main ( -- ) 
  glfwInit drop
  GLFW_CONTEXT_VERSION_MAJOR 4 glfwWindowHint
  GLFW_CONTEXT_VERSION_MINOR 6 glfwWindowHint
  GLFW_OPENGL_PROFILE GLFW_OPENGL_CORE_PROFILE glfwWindowHint

  800 600 "lmeow" utf8 string>alien f f glfwCreateWindow
  dup glfwMakeContextCurrent
  [ dup glfwWindowShouldClose 0 = ] [
    0.2 0.3 0.3 1.0 glClearColor
    GL_COLOR_BUFFER_BIT glClear

    dup glfwSwapBuffers
    glfwPollEvents
  ] while
  glfwDestroyWindow
  glfwTerminate
;


MAIN: main
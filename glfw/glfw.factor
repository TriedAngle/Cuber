USING: accessors alien.strings arrays classes.tuple
classes.tuple.parser io.encodings.utf8 kernel lexer literals
math namespaces sequences slots.syntax strings ;
QUALIFIED-WITH: glfw.ffi ffi
QUALIFIED-WITH: alien.c-types c
IN: glfw

SYMBOL: windows 

: setup-windows? ( -- ? ) 
  windows get-global [ f ] [ V{ } clone windows set-global t ] if ;

: ?init ( -- ) 
  setup-windows? [ ffi:glfwInit drop ] [ ] if ;

: ?terminate ( -- ) 
  windows get-global empty? [ ffi:glfwTerminate f windows set-global ] when ;

: push-window ( GLFWwindow -- )
  ?init windows get-global push ;

: pop-window ( GLFwindow -- )
  windows get-global remove! drop ;

: allocate-window ( width height title monitor share -- GLFWwindow ) 
  ffi:glfwCreateWindow ;

: window-hint ( hint value -- ) 
  ffi:glfwWindowHint ;

TUPLE: window-attributes
  { dim array }
  { title string }
  { monitor initial: f }
  { share initial: f }
  { version array initial: { 3 3 } } 
  { profile fixnum initial: $ ffi:GLFW_OPENGL_CORE_PROFILE } ;

: set-gl-hints ( version profile -- ) 
  [ [ first ] [ second ] bi ] dip 3array
  { $ ffi:GLFW_CONTEXT_VERSION_MAJOR $ ffi:GLFW_CONTEXT_VERSION_MINOR 
    $ ffi:GLFW_OPENGL_PROFILE }
  [ window-hint ] 2each ;

TUPLE: window 
  { attributes window-attributes }
  underlying ;

: new-window ( window-attributes -- window )
  ?init ffi:glfwDefaultWindowHints
  dup slots[ version profile ] set-gl-hints
  dup slots[ dim title monitor share ] 
  [ [ [ first ] [ second ] bi ] dip utf8 string>alien ] 2dip
  ffi:glfwCreateWindow dup push-window window boa ;

: close ( window -- ) 
  underlying>> dup ffi:glfwDestroyWindow pop-window ?terminate ;

: should-close? ( window -- ? ) 
  underlying>> ffi:glfwWindowShouldClose 1 = ;

: set-context ( window -- ) 
  underlying>> ffi:glfwMakeContextCurrent ;

: swap-buffers ( window -- ) 
  underlying>> ffi:glfwSwapBuffers ;

: poll-events ( -- ) 
  ffi:glfwPollEvents ;

: run-window ( window quot -- )
  [ dup set-context ] dip
  [ [ dup should-close? ] dip swap not ] [ 
    dup call( -- )
    [ dup swap-buffers ] dip
    poll-events
  ] while drop close
; inline

: parse-window-attributes ( -- attributes ) 
  "{" expect window-attributes dup all-slots parse-tuple-literal-slots ;

SYNTAX: WINDOW:
  parse-window-attributes new-window suffix! ;

! : main ( -- ) 
!  glfwInit drop
!  GLFW_CONTEXT_VERSION_MAJOR 4 glfwWindowHint
!  GLFW_CONTEXT_VERSION_MINOR 6 glfwWindowHint
!  GLFW_OPENGL_PROFILE GLFW_OPENGL_CORE_PROFILE glfwWindowHint
!
!  800 600 "hello" utf8 string>alien f f glfwCreateWindow
!  dup glfwMakeContextCurrent
!  [ dup glfwWindowShouldClose 0 = ] [
!    0.2 0.3 0.3 1.0 glClearColor
!    GL_COLOR_BUFFER_BIT glClear
!
!    dup glfwSwapBuffers
!    glfwPollEvents
!  ] while
!  glfwDestroyWindow
!  glfwTerminate
! ;

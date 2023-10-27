USING: accessors alien.strings arrays classes.tuple
classes.tuple.parser io.encodings.utf8 kernel lexer literals
math namespaces sequences slots.syntax strings threads ;
QUALIFIED-WITH: glfw.ffi ffi
QUALIFIED-WITH: alien.c-types c
! FROM: ui.private => ui-running ;
IN: glfw

SYMBOL: ui-running

SYMBOL: windows 
: windows* ( -- windows ) windows get-global ;

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
  ffi:glfwCreateWindow dup push-window 
  dup ffi:glfwMakeContextCurrent
  window boa ;

: close ( window -- ) 
  underlying>> dup ffi:glfwDestroyWindow pop-window ?terminate ;

: should-close? ( window -- ? ) 
  underlying>> ffi:glfwWindowShouldClose 1 = ;

: set-context ( window -- ) 
  underlying>> ffi:glfwMakeContextCurrent ;

: set-should-close ( window ? -- ) 
  [ underlying>> ] dip [ 1 ] [ 0 ] if ffi:glfwSetWindowShouldClose ;

: swap-buffers ( window -- ) 
  underlying>> ffi:glfwSwapBuffers ;

: poll-events ( -- ) 
  ffi:glfwPollEvents ;

: set-key-callback ( window cb -- ) 
  [ underlying>> ] dip ffi:glfwSetKeyCallback drop ;

: run-window-sync ( window quot: ( window -- ) -- )
  swap [ dup should-close? not ] [
    dup set-context
    2dup swap call( window -- )
    yield
    dup swap-buffers
    poll-events
  ] while nip close ;

: run-window-async ( window quot: ( window -- ) -- ) 
  '[ _ _ swap
    [ dup should-close? not ] [
      dup set-context
      2dup swap call( window -- )
      yield
      dup swap-buffers
      poll-events
    ] while close drop stop
  ] "glfw" spawn drop ;

: run-window ( window quot: ( window -- ) -- )
  ui-running get-global [
    run-window-async
  ] [
    t ui-running set-global
    run-window-sync  
  ] if ;

: parse-window-attributes ( -- attributes ) 
  "{" expect window-attributes dup all-slots parse-tuple-literal-slots ;

SYNTAX: WINDOW:
  parse-window-attributes new-window suffix! ;


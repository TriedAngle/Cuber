USING: accessors destructors io.encodings.utf8
io.files kernel opengl.gl opengl.shaders slots.syntax ;
IN: sdfui.shaders

CONSTANT: vertex-path   "vocab:sdfui/shaders/shader.vert"
CONSTANT: fragment-path "vocab:sdfui/shaders/shader.frag"
CONSTANT: compute-path  "vocab:sdfui/shaders/shader.comp"

TUPLE: sdfui-shaders < disposable 
  vertex fragment program ;

: <sdfui-shaders> ( -- sdfui-shaders )
  sdfui-shaders new-disposable
  vertex-path utf8 file-contents >>vertex
  fragment-path utf8 file-contents >>fragment
  dup slots[ vertex fragment ] <simple-gl-program>
  >>program ;

M: sdfui-shaders dispose*
  program>> glDeleteProgram ;

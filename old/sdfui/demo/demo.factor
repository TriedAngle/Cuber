USING: accessors alien alien.c-types alien.strings alien.syntax
calendar colors combinators glfw io.encodings.utf8 kernel
literals namespaces opengl opengl.gl opengl.gl.windows pressa
pressa.constants pressa.glfw prettyprint sdfui sequences threads
multiline windows.types opengl.shaders io.files ;
IN: sdfui.demo

: main ( -- )
  [
  T{ window-attributes 
    { dim { 1280 720 } }
    { title "sdfui demo" }
    { version { 4 6 } }
  } new-window

  dup set-pressa-callbacks

  dup <sdfui-ctx>

  [| ctx | [| window | ctx {
     ! [ drop COLOR: seagreen gl-clear ]
      [ sdfui-record ]
      [ 300 500 100 COLOR: red f sdfui>circle ]
      [ 900 300 60 COLOR: blue f sdfui>circle ]
      [ 300 350 100 50 COLOR: red usmin 0.2 <merge> sdfui>box ]
      [ 1.5 COLOR: white sdfui>outline ]
      [ 100 600 "hello 猫🐱 044 XD" 
        42 { "Arial.ttf" "msyh.ttc" "seguiemj.ttf" } 
        COLOR: white f sdfui>text ]
      [ 200 300 "i am in pain owo" 
        42 { "comici.ttf" } 
        COLOR: white f sdfui>text ]
      [ sdfui-render ]
    } cleave
    keyEscape released? [ window t set-should-close ] when
    pressa-flush
    yield
  ] run-window-sync ] call drop 
] "lol " spawn drop [ 10 milliseconds sleep t ] loop ;
MAIN: main


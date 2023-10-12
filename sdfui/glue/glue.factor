USING: accessors alien alien.c-types alien.libraries
alien.strings alien.syntax alien.utilities classes.struct
combinators destructors io.backend io.encodings.utf8 kernel libc
math sequences specialized-arrays.instances.alien.c-types.uchar
system ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.glue

<< "glyphers" {
  { [ os windows? ] [ "glyphers.dll" ] }
  { [ os linux?   ] [ "libglyphers.so" ] }
  [ drop "other os not implemented yet" throw ]
} cond cdecl add-library >>

STRUCT: RasterizationResult
  { width usize }
  { height usize }
  { length usize }
  { pointer c:uchar* } ;

LIBRARY: glyphers

FUNCTION: RasterizationResult rasterize ( 
  c:char* text, 
  c:char** fonts, 
  c:size_t fonts_count, 
)

FUNCTION: c:void deallocate_rasterization ( c:char* ptr, c:size_t size )

TUPLE: rasterization < disposable
  { data uchar-array }
  { width fixnum }
  { height fixnum } ;

M: rasterization dispose*
  data>> dup length deallocate_rasterization ; 

: <rasterization> ( result -- rasterization )
  { [ width>> ] [ height>> ] [ pointer>> ] [ length>> ] } cleave
  <direct-uchar-array>
  rasterization new-disposable [
  [ data<< ] [ height<< ] [ width<< ] tri ] keep ;

: rasterize-text ( text fonts -- rasterization ) 
  [ utf8 string>alien ] dip 
  utf8 strings>alien [ dup length 1 - rasterize ] keep
  [ [ &free drop ] each ] with-destructors 
  <rasterization> ;

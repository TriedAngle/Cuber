USING: alien alien.c-types alien.libraries alien.syntax
classes.struct combinators io.backend kernel system ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.glue

<< "glyphers" {
  { [ os windows? ] [ "vocab:sdfui/glue/glyphers.dll" normalize-path ] }
  { [ drop "other os not implemented yet" throw ] }
} cond cdecl add-library >>

STRUCT: RasterizationNormResult
  { width usize }
  { height usize }
  { length usize }
  { pointer c:float* } ;

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

FUNCTION: RasterizationNormResult rasterize_norm ( 
  c:char* text, 
  c:char** fonts, 
  c:size_t fonts_count, 
)

FUNCTION: c:void deallocate_rasterization_norm ( c:float* ptr, c:size_t size )

FUNCTION: c:void deallocate_rasterization ( c:char* ptr, c:size_t size )


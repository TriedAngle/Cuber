USING: accessors alien.data arrays assocs destructors freetype
kernel math sdfui.cache sdfui.utils sequences strings ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.fonts

TUPLE: font-key string font ;

: <fonted-key> ( string font -- bitmap-key ) font-key boa ;


TUPLE: font-string < disposable 
  { value string }
  { font string }
  { face fixnum } ;

M: font-string dispose* face>> FT_Done_Face ;

: <font-string> ( string font face -- font-string ) 
  font-string new-disposable
  >>face >>font >>value ; 

TUPLE: sdfui-fonts < disposable
  { library fixnum } 
  { fonts hashcache } ;

M: sdfui-fonts dispose* library>> FT_Done_FreeType ;

: <sdfui-fonts> ( -- fonts )
  sdfui-fonts new-disposable
  0 c:int [ FT_Init_FreeType drop ] ref >>library ! do not ignore error
  69 42 <hashcache> >>fonts ;

 
 M: sdfui-fonts at* fonts>> at* ;

: sdfui-age ( fonts -- ) 
  [ fonts>> delete-next [ dispose ] each ] 
  [ fonts>> age-hashcache ] bi ;

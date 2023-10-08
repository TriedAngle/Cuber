USING: accessors alien.data arrays assocs destructors freetype
kernel math sdfui.cache sdfui.utils sequences strings ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.fonts

TUPLE: font-key string size font ;

: <font-key> ( string size font -- bitmap-key ) font-key boa ;


TUPLE: font-string < disposable 
  { value string }
  { size fixnum }
  { font string }
  { face fixnum } ;

M: font-string dispose* face>> FT_Done_Face ;

: <font-string> ( string size font face -- font-string ) 
  font-string new-disposable
  >>face >>font >>size >>value ; 

TUPLE: sdfui-fonts < disposable
  { library fixnum } 
  { fonts hashcache } ;

M: sdfui-fonts dispose* 
  [ fonts>> >alist dispose-each ]
  [ library>> FT_Done_FreeType ] bi ;

: <sdfui-fonts> ( -- fonts )
  sdfui-fonts new-disposable
  0 c:int [ FT_Init_FreeType throw ] ref >>library
  69 42 <hashcache> >>fonts ;
 
M: sdfui-fonts at* fonts>> at* ;

M: sdfui-fonts set-at fonts>> set-at ;

:: add-string* ( str size font sdfui-fonts -- font-string ) 
  sdfui-fonts library>> font 0 0 c:int [ FT_New_Face throw ] ref :> face
  str size font face <font-string> 
  [ str size font <font-key> sdfui-fonts set-at ] keep ;

: add-string ( string size font sdfui-fonts -- font-string )
  [ 3dup <font-key> ] dip dup swapd
  at dup [ [ 4drop ] dip ] [ drop add-string* ] if ;

: sdfui-age ( fonts -- ) 
  [ fonts>> delete-next dispose-each ] 
  [ fonts>> age-hashcache ] bi ;

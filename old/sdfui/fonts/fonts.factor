USING: accessors alien.data arrays assocs combinators
destructors freetype kernel math sdfui.cache sdfui.glue
sdfui.utils sequences strings ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.fonts

TUPLE: font-key string size fonts ;

: <font-key> ( string size fonts -- bitmap-key ) font-key boa ;


TUPLE: font-string < disposable 
  { value string }
  { size fixnum }
  { fonts sequence }
  { rasta rasterization } ;

M: font-string dispose* rasta>> dispose ;

: <font-string> ( string size fonts -- font-string ) 
  font-string new-disposable [ {
    [ fonts<< ] [ size<< ] [ value<< ]
  } cleave ] keep dup [ value>> ] [ fonts>> ] bi 
  rasterize-text >>rasta ;

TUPLE: sdfui-fonts < disposable
  { fonts hashcache } ;

M: sdfui-fonts dispose* 
  fonts>> >alist dispose-each ;

: <sdfui-fonts> ( -- fonts )
  sdfui-fonts new-disposable
  69 42 <hashcache> >>fonts ; ! funni numbers hehee
 
M: sdfui-fonts at* fonts>> at* ;

M: sdfui-fonts set-at fonts>> set-at ;

: add-string* ( string size fonts sdfui-fonts -- font-string ) 
  [ [ <font-string> dup ] [ <font-key> ] 3bi ] dip set-at ;

: add-string ( string size fonts sdfui-fonts -- font-string )
  [ 3dup <font-key> ] dip dup swapd
  at dup [ [ 4drop ] dip ] [ drop add-string* ] if ;

: sdfui-age ( fonts -- ) 
  [ fonts>> delete-next dispose-each ] 
  [ fonts>> age-hashcache ] bi ;

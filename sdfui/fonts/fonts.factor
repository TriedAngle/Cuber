USING: alien.data freetype kernel math sdfui.utils sdfui.cache ;
QUALIFIED-WITH: alien.c-types c
IN: sdfui.fonts

TUPLE: sdfui-fonts
  { library fixnum } 
  { bitmaps hashcache } ;

: <sdfui-fonts> ( -- sdfui-fonts ) 
  0 c:int [ FT_Init_FreeType drop ] ref ! do not ignore error
  69 42 <hashcache> ! funni numbers
  sdfui-fonts boa ;



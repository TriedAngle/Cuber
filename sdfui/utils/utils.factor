! Copyright (C) 2023 Your name.
! See https://factorcode.org/license.txt for BSD license.
USING: alien.data kernel math ;
IN: sdfui.utils

: ref ( value c-type quot -- value ) 
  -rot [ <ref> ] keep [ drop swap call ] 2keep deref ; inline

: round-to ( n multiple -- n' )
  2dup mod [ drop ] [ - + ] if-zero ;


USING: accessors arrays assocs hashtables kernel math sequences ;
IN: sdfui.cache

TUPLE: hashcache-entry value age ;

: <hashcache-entry> ( value -- entry ) 0 hashcache-entry boa ;

TUPLE: hashcache assoc max-age ;

M: hashcache at*
  assoc>> at* [ [ 0 >>age value>> ] ?call ] dip ;

M: hashcache assoc-size assoc>> assoc-size ;

M: hashcache >alist assoc>> [ value>> ] { } assoc-map-as ;

M: hashcache set-at
  [ <hashcache-entry> ] 2dip assoc>> set-at ;

M: hashcache delete-at
  assoc>> delete-at ;

M: hashcache clear-assoc assoc>> clear-assoc ;

: <hashcache> ( init-size max-age -- hashcache ) 
  [ <hashtable> ] dip hashcache boa ;

: age-hashcache ( cache -- ) 
  dup [ assoc>> >alist ] [ max-age>> ] bi '[ second age>> _ <= ] filter
  [ [ first ] [ second ] bi [ 1 + ] change-age 2array ] map
  >hashtable >>assoc drop ;

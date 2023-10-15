USING: accessors arrays combinators kernel namespaces
pressa.constants sequences sets vectors ;
IN: pressa

SYMBOL: pressa

: pressa* ( -- pressa ) pressa get-global ;

: mod? ( mod -- ? ) { modControl modShift modAlt modCommand modCaps modNumlock } in? ;

: mod<|>keys ( mod -- keyL keyR ) { 
  { [ dup modControl = ] [ keyLControl keyRControl ] }
  { [ dup modShift = ] [ keyLShift keyRShift ] }
  { [ dup modAlt = ] [ keyLAlt keyRAlt ] }
  { [ dup modCommand = ] [ keyLCommand keyRCommand ] }
} cond nipd ;

TUPLE: input 
  { mods vector }
  { pressed vector }
  { hold vector }
  { released vector } 
  { cursor array } ;

: <input> ( -- input ) 
  V{ } clone V{ } clone V{ } clone V{ } clone { 0.0 0.0 } input boa ;

: setup-pressa ( -- ) 
  <input> pressa set-global ;

pressa* [ setup-pressa ] unless

:: test-input? ( key keys -- ? ) 
  key dup mod? [ mod<|>keys ] [ dup ] if [ keys in? ] bi@ or ; inline

GENERIC: pressed? ( key(s) -- ? )
GENERIC: active? ( key(s) -- ? )

: set-mods ( seq -- ) pressa* mods<< ;

M: object pressed? pressa* pressed>> test-input? ;

: hold? ( key -- ? ) pressa* hold>> test-input? ;

: released? ( key -- ? ) pressa* released>> test-input? ;

M: object active? [ pressed? ] [ hold? ] bi or ;

M: sequence active? t [ active? and ] reduce ;

M: sequence pressed?
  [ active? ] [ t [ hold? and ] reduce not ] bi and ;

: hold ( key -- ) 
  [ pressa* pressed>> delete ] 
  [ dup hold? [ drop ] [ pressa* hold>> push ] if ] bi ;

: press ( key -- )
  dup active? [ hold ] [ pressa* pressed>> push ] if ;

: release ( key -- )
  [ dup active? [ pressa* released>> push ] [ drop ] if ] 
  [ pressa* [ pressed>> delete ] [ hold>> delete ] 2bi ] bi ;

: press-combo ( keys -- ) '[ press ] each ;

: pressed>hold ( -- ) pressa* [ hold>> ] [ pressed>> ] bi [ append! drop ] keep
  delete-all ;

: clear-released ( -- ) pressa* released>> delete-all ;

: pressa-flush ( -- ) pressed>hold clear-released ;


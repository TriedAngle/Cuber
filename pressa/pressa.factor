USING: accessors arrays combinators kernel sequences sets
vectors ;
IN: pressa

SYMBOLS:
  keyUnknown keyEscape keyPrt keyScr KeyPause 
  keyIns KeyHome keyPgUp keyDel keyEnd keyPgDown
  keyUp keyDown keyLeft keyRight KeyTab KeyEnter KeyBack

  keyF1 keyF2 keyF3 keyF4  keyF5  keyF6
  keyF7 keyF8 keyF9 keyF10 keyF11 keyF12
  keyGrave keyMinus keyEquals 
  keyLBrack keyRBrack keySemi keyApo keyBSlashL keyBackSlash keyBackSlashL
  keyComma keyDot keySlash
  keyLShift keyRShift keyLControl keyRControl keyLAlt keyRAlt 
  keyWin keySpace keyFn 
  
  key0 key1 key2 key3 key4 key5 key6 key7 key8 key9
  keyA keyB keyC keyD keyE keyF keyG keyH keyI 
  keyJ keyK keyL keyM keyN keyO keyP keyQ keyR 
  keyS keyT keyU keyV keyW keyX keyY keyZ

  modControl modShift modAlt modWin
;

: mod? ( mod -- ? ) { modControl modShift modAlt modWin } in? ;

: mod<|>keys ( mod -- keyL keyR ) { 
  { [ dup modControl = ] [ keyLControl keyRControl ] }
  { [ dup modShift = ] [ keyLShift keyRShift ] }
  { [ dup modAlt = ] [ keyLAlt keyRAlt ] }
  { [ dup modWin = ] [ keyWin keyWin ] }
} cond nipd ;

TUPLE: input 
  { pressed vector }
  { hold vector }
  { released vector } 
  { cursor array } ;

: <input> ( -- input ) 
  V{ } clone V{ } clone V{ } clone { 0.0 0.0 } input boa ;

:: test-input? ( key keys -- ? ) 
  key dup mod? [ mod<|>keys ] [ dup ] if [ keys in? ] bi@ or ; inline

GENERIC#: pressed? 1 ( key(s) input -- ? )
GENERIC#: active? 1 ( key(s) input -- ? )

M: object pressed? pressed>> test-input? ;

: hold? ( key input -- ? ) hold>> test-input? ;

: released? ( key input -- ? ) released>> test-input? ;

M: object active? [ pressed? ] [ hold? ] 2bi or ;

M: sequence active? t swap '[ _ active? and ] reduce ;

M: sequence pressed?
  [ active? ] [ t swap '[ _ hold? and ] reduce not ] 2bi and ;

: press>hold ( key input -- ) 
  [ pressed>> delete ] [ 2dup hold? [ 2drop ] [ hold>> push ] if ] 2bi ;

: press ( key input -- )
  2dup pressed? [ press>hold ] [ pressed>> push ] if ;

: release ( key input -- )
  [ 2dup active? [ released>> push ] [ 2drop ] if ] 
  [ [ pressed>> delete ] [ hold>> delete ] 2bi ] 2bi ;

: press-combo ( keys input -- ) '[ _ press ] each ;

: clear-released ( input -- ) released>> delete-all ;


USING: assocs combinators glfw glfw.ffi io kernel literals
namespaces pressa pressa.constants prettyprint ;
IN: pressa.glfw

INITIALIZED-SYMBOL: glfw>pressa|mappings [ H{
  { $ GLFW_KEY_UNKNOWN keyUnknown }
  { $ GLFW_KEY_SPACE keySpace }
  { $ GLFW_KEY_APOSTROPHE keyApo }
  { $ GLFW_KEY_COMMA keyComma }
  { $ GLFW_KEY_MINUS keyMinus }
  { $ GLFW_KEY_PERIOD keyDot }
  { $ GLFW_KEY_SLASH keySlash }
  { $ GLFW_KEY_SEMICOLON keySemi }
  { $ GLFW_KEY_EQUAL keyEqual }
  { $ GLFW_KEY_LEFT_BRACKET keyLBrack }
  { $ GLFW_KEY_BACKSLASH keyBackSlash }
  { $ GLFW_KEY_RIGHT_BRACKET keyRBrack }
  { $ GLFW_KEY_GRAVE_ACCENT keyGrave }
  { $ GLFW_KEY_ESCAPE keyEscape }
  { $ GLFW_KEY_ENTER keyEnter }
  { $ GLFW_KEY_TAB keyTab }
  { $ GLFW_KEY_BACKSPACE keyBack }
  { $ GLFW_KEY_INSERT keyIns }
  { $ GLFW_KEY_DELETE keyDel }
  { $ GLFW_KEY_RIGHT keyRight }
  { $ GLFW_KEY_LEFT keyLeft }
  { $ GLFW_KEY_DOWN keyDown }
  { $ GLFW_KEY_UP keyUp }
  { $ GLFW_KEY_PAGE_UP keyPgUp }
  { $ GLFW_KEY_PAGE_DOWN keyPgDown }
  { $ GLFW_KEY_HOME keyHome }
  { $ GLFW_KEY_END keyEnd }
  { $ GLFW_KEY_CAPS_LOCK keyCaps }
  { $ GLFW_KEY_SCROLL_LOCK keyScrLk }
  { $ GLFW_KEY_NUM_LOCK keyNumlock }
  { $ GLFW_KEY_PRINT_SCREEN keyPrtSc }
  { $ GLFW_KEY_PAUSE keyPause }
  { $ GLFW_KEY_LEFT_SHIFT keyLShift }
  { $ GLFW_KEY_LEFT_CONTROL keyLControl }
  { $ GLFW_KEY_LEFT_ALT keyLAlt }
  { $ GLFW_KEY_LEFT_SUPER keyLCommand }
  { $ GLFW_KEY_RIGHT_SHIFT keyRShift }
  { $ GLFW_KEY_RIGHT_CONTROL keyRControl }
  { $ GLFW_KEY_RIGHT_ALT keyRAlt }
  { $ GLFW_KEY_RIGHT_SUPER keyRCommand }

  { $ GLFW_KEY_F1 keyF1 }
  { $ GLFW_KEY_F2 keyF2 }
  { $ GLFW_KEY_F3 keyF3 }
  { $ GLFW_KEY_F4 keyF4 }
  { $ GLFW_KEY_F5 keyF5 }
  { $ GLFW_KEY_F6 keyF6 }
  { $ GLFW_KEY_F7 keyF7 }
  { $ GLFW_KEY_F8 keyF8 }
  { $ GLFW_KEY_F9 keyF9 }
  { $ GLFW_KEY_F10 keyF10 }
  { $ GLFW_KEY_F11 keyF11 }
  { $ GLFW_KEY_F12 keyF12 }
  { $ GLFW_KEY_F13 keyF13 }
  { $ GLFW_KEY_F14 keyF14 }
  { $ GLFW_KEY_F15 keyF15 }
  { $ GLFW_KEY_F16 keyF16 }
  { $ GLFW_KEY_F17 keyF17 }
  { $ GLFW_KEY_F18 keyF18 }
  { $ GLFW_KEY_F19 keyF19 }
  { $ GLFW_KEY_F20 keyF20 }
  { $ GLFW_KEY_F21 keyF21 }
  { $ GLFW_KEY_F22 keyF22 }
  { $ GLFW_KEY_F23 keyF23 }
  { $ GLFW_KEY_F24 keyF24 }
  { $ GLFW_KEY_F25 keyF25 }

  { $ GLFW_KEY_0 key0 }
  { $ GLFW_KEY_1 key1 }
  { $ GLFW_KEY_2 key2 }
  { $ GLFW_KEY_3 key3 }
  { $ GLFW_KEY_4 key4 }
  { $ GLFW_KEY_5 key5 }
  { $ GLFW_KEY_6 key6 }
  { $ GLFW_KEY_7 key7 }
  { $ GLFW_KEY_8 key8 }
  { $ GLFW_KEY_9 key9 }
  { $ GLFW_KEY_A keyA }
  { $ GLFW_KEY_B keyB }
  { $ GLFW_KEY_C keyC }
  { $ GLFW_KEY_D keyD }
  { $ GLFW_KEY_E keyE }
  { $ GLFW_KEY_F keyF }
  { $ GLFW_KEY_G keyG }
  { $ GLFW_KEY_H keyH }
  { $ GLFW_KEY_I keyI }
  { $ GLFW_KEY_J keyJ }
  { $ GLFW_KEY_K keyK }
  { $ GLFW_KEY_L keyL }
  { $ GLFW_KEY_M keyM }
  { $ GLFW_KEY_N keyN }
  { $ GLFW_KEY_O keyO }
  { $ GLFW_KEY_P keyP }
  { $ GLFW_KEY_Q keyQ }
  { $ GLFW_KEY_R keyR }
  { $ GLFW_KEY_S keyS }
  { $ GLFW_KEY_T keyT }
  { $ GLFW_KEY_U keyU }
  { $ GLFW_KEY_V keyV }
  { $ GLFW_KEY_W keyW }
  { $ GLFW_KEY_X keyX }
  { $ GLFW_KEY_Y keyY }
  { $ GLFW_KEY_Z keyZ }

  { $ GLFW_MOD_SHIFT modShift }
  { $ GLFW_MOD_CONTROL modControl }
  { $ GLFW_MOD_ALT modAlt }
  { $ GLFW_MOD_SUPER modCommand }
  { $ GLFW_MOD_CAPS_LOCK modCaps }
  { $ GLFW_MOD_NUM_LOCK modNumlock }

  { $ GLFW_MOUSE_BUTTON_1 mouseLeft }
  { $ GLFW_MOUSE_BUTTON_2 mouseRight }
  { $ GLFW_MOUSE_BUTTON_3 mouseMiddle }
  { $ GLFW_MOUSE_BUTTON_4 mouse4 }
  { $ GLFW_MOUSE_BUTTON_5 mouse5 }
  { $ GLFW_MOUSE_BUTTON_6 mouse6 }
  { $ GLFW_MOUSE_BUTTON_7 mouse7 }
  { $ GLFW_MOUSE_BUTTON_8 mouse8 }
  [ drop keyUnmapped ]
} ]

: glfw>pressa|mappings* ( key -- mapped ) 
  glfw>pressa|mappings get-global at ;

: pressa-key-callback ( -- alien ) 
  [| window key scancode action mods |
    key glfw>pressa|mappings* action {
      { $ GLFW_RELEASE [ release ] }
      { $ GLFW_PRESS   [ press ] }
      { $ GLFW_REPEAT  [ press ] }
    } case
  ] GLFWkeyfun ;


: set-pressa-callbacks ( window -- ) {
  [ pressa-key-callback set-key-callback ]
} cleave ;

! this works but has to be implemented
! << 
!   "glyphers" image-path "factor.image" "" replace 
!   "work/sdfui/glue/" append
!   "glyphers.dll" append cdecl add-library 
! >>

! LIBRARY: glyphers

! FUNCTION: c:int load_opengl_function_pointers ( c:char* path )
! "opengl32.dll" ascii string>alien load_opengl_function_pointers
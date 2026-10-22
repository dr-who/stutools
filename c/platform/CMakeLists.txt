add_custom_target(spit_version
  COMMAND ${CMAKE_COMMAND} -DSOURCE_DIR=${PROJECT_SOURCE_DIR}/spit
    -DOUTPUT_FILE=${CMAKE_CURRENT_BINARY_DIR}/spit-version.h
    -P ${PROJECT_SOURCE_DIR}/spit/GenerateVersion.cmake
  BYPRODUCTS ${CMAKE_CURRENT_BINARY_DIR}/spit-version.h VERBATIM)
add_executable(spit spit_native.c native_io.c ${CMAKE_CURRENT_BINARY_DIR}/spit-version.h)
add_dependencies(spit spit_version)
target_include_directories(spit PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
if(MSVC)
  target_compile_definitions(spit PRIVATE _CRT_SECURE_NO_WARNINGS)
  target_compile_options(spit PRIVATE /W4)
else()
  target_compile_options(spit PRIVATE -Wall -Wextra -Wpedantic)
  if(NOT WIN32)
    target_link_libraries(spit PRIVATE m)
  endif()
endif()
install(PROGRAMS "$<TARGET_FILE:spit>" DESTINATION ${CMAKE_INSTALL_BINDIR} RENAME "spitc${CMAKE_EXECUTABLE_SUFFIX}")
install(FILES ${PROJECT_SOURCE_DIR}/LICENSE ${PROJECT_SOURCE_DIR}/platform/README.md
  DESTINATION ${CMAKE_INSTALL_DOCDIR})
add_test(NAME nativeSmoke COMMAND ${CMAKE_COMMAND} -DSPIT=$<TARGET_FILE:spit>
  -DTEST_DIR=${CMAKE_CURRENT_BINARY_DIR}/smoke -P ${CMAKE_CURRENT_SOURCE_DIR}/Smoke.cmake)
set_tests_properties(nativeSmoke PROPERTIES LABELS smoke)
set(CPACK_PACKAGE_NAME stutools)
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_VENDOR "Stuart Inglis")
set(CPACK_GENERATOR ZIP)
if(UNIX)
  set(CPACK_GENERATOR TGZ)
endif()
include(CPack)

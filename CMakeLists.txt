cmake_minimum_required (VERSION 3.0)
project (stutools C)

include(${CMAKE_CURRENT_SOURCE_DIR}/utils/Version.cmake)

enable_testing()

add_subdirectory(iotests)
add_subdirectory(spit)
add_subdirectory(utils)
add_subdirectory(report)




find_package(Python3 COMPONENTS Interpreter QUIET)
if(Python3_Interpreter_FOUND)
  add_test(NAME spitSafety COMMAND ${Python3_EXECUTABLE}
    ${CMAKE_CURRENT_SOURCE_DIR}/tests/test_spit_safety.py $<TARGET_FILE:spit>)
endif()

cmake_minimum_required (VERSION 3.0)
project (stutools C)

include(${CMAKE_CURRENT_SOURCE_DIR}/utils/Version.cmake)

enable_testing()

add_subdirectory(iotests)
add_subdirectory(spit)
add_subdirectory(utils)
add_subdirectory(report)




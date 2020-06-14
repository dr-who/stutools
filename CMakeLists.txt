cmake_minimum_required (VERSION 2.8)
project (stutools C)

include(${CMAKE_CURRENT_SOURCE_DIR}/utils/Version.cmake)

add_subdirectory(iotests)
add_subdirectory(spit)
add_subdirectory(utils)
add_subdirectory(report)




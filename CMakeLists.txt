cmake_minimum_required (VERSION 2.6)
project (stutools)

execute_process(COMMAND git rev-parse HEAD
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
  OUTPUT_VARIABLE GIT_COMMIT_HASH
  OUTPUT_STRIP_TRAILING_WHITESPACE)

execute_process(COMMAND git log -1 --format=%cd
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
  OUTPUT_VARIABLE GIT_COMMIT_DATE
  OUTPUT_STRIP_TRAILING_WHITESPACE)

add_definitions( -DVERSION="${GIT_COMMIT_HASH} - ${GIT_COMMIT_DATE}" )
message(STATUS "Building stutools ${GIT_COMMIT_HASH} (${GIT_COMMIT_DATE})")

add_subdirectory(iotests)
add_subdirectory(spit)
add_subdirectory(utils)



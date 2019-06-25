
cmake_minimum_required (VERSION 2.6)
project (stutools)

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wall -pedantic --std=c99 -O2" )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")

add_library(spitlib STATIC positions.c devices.c utils.c diskStats.c logSpeed.c aioRequests.c jobType.c histogram.c fuzz.c blockVerify.c)

add_executable(spit spit.c)
target_link_libraries(spit spitlib m aio pthread)

add_executable(spitchecker verify.c)
target_link_libraries(spitchecker spitlib m aio pthread)

add_executable(bdinfo bdinfo.c)
target_link_libraries(bdinfo spitlib m aio pthread)

add_executable(dtest directories.c)
target_link_libraries(dtest spitlib m aio pthread)

add_executable(hist hist.c)
target_link_libraries(hist spitlib m)

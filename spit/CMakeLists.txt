
cmake_minimum_required (VERSION 2.6)
project (spit)

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wall -pedantic --std=c99 -O2" )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")

add_library(spitlib STATIC positions.c devices.c cigar.c utils.c blockVerify.c diskStats.c logSpeed.c aioRequests.c jobType.c)

add_executable(spit spit.c)
target_link_libraries(spit spitlib m aio pthread)

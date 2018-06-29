
cmake_minimum_required (VERSION 2.6)
project (stutools)

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wall -pedantic --std=c99" )

add_library(common STATIC positions.c devices.c cigar.c utils.c blockVerify.c diskStats.c logSpeed.c aioRequests.c)

add_executable(aioRWTest aioRWTest.c)
target_link_libraries(aioRWTest common m aio pthread)

add_executable(verify verify.c)
target_link_libraries(verify common m aio pthread)


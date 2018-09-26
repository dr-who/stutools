
cmake_minimum_required (VERSION 2.6)
project (stutools)

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wall -pedantic --std=c99 -O2" )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")

add_library(stutools STATIC positions.c devices.c cigar.c utils.c blockVerify.c diskStats.c logSpeed.c aioRequests.c)

add_executable(verify verify.c)
target_link_libraries(verify stutools m aio pthread)

add_executable(repeat repeat.c)
target_link_libraries(repeat stutools m aio pthread)

add_executable(aioMulti aioMulti.c)
target_link_libraries(aioMulti stutools m aio pthread)

#add_executable(checkDisks checkDisks.c)
#target_link_libraries(checkDisks stutools m aio pthread)

add_executable(ramEater ramEater.c)
target_link_libraries(ramEater stutools)

add_executable(aioRWTest aioRWTest.c)
target_link_libraries(aioRWTest stutools m aio pthread)

add_executable(trim trim.c)
target_link_libraries(trim stutools)

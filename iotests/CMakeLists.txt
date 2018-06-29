
cmake_minimum_required (VERSION 2.6)
project (stutools)

add_executable(aioRWTest aioRWTest.c aioRequests.c logSpeed.c utils.c diskStats.c positions.c cigar.c devices.c blockVerify.c)
target_link_libraries(aioRWTest m aio pthread)

add_executable(verify verify.c positions.c devices.c cigar.c utils.c blockVerify.c)
target_link_libraries(verify m aio pthread)


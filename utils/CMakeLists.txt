cmake_minimum_required (VERSION 3.0)
project (stutools)

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wall -pedantic --std=c99 -O2" )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")

add_executable(median median.c)

add_executable(dist dist.c)

add_executable(combo combo.c)

add_executable(ramtester ramtester.c)

add_executable(raidfailures raidfailures.c deviceProbs.c)
target_link_libraries(raidfailures m)

add_executable(raidsimulation raidsimulation.c deviceProbs.c)
target_link_libraries(raidsimulation m)






